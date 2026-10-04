//! Experimental storage writes checked in memory; not a shipped game build.
use crate::{
    font_loader, loader_restore,
    media::sha256,
    z80_writes::{CodeSource, Verifier},
};
use expected_write::{
    ExpectedWrite, ImageRegion, MachineCodeProvenance, RegionKind, WriteIntent, WritePlan,
};
use retro_z80::{Assembler, Instruction, PROFILE_ID};
use serde::Serialize;
use std::error::Error;

#[derive(Clone, Copy, PartialEq)]
pub enum Probe {
    Storage,
    FontLoad,
    PageOneRenderer,
    MainMenus,
    MainText,
}

#[derive(Serialize)]
struct PlannedWrite {
    id: String,
    offset: usize,
    expected_hex: String,
    replacement_hex: String,
    assembly_source_id: Option<String>,
    cpu_origin: Option<u16>,
}

#[derive(Serialize)]
pub struct Receipt {
    #[serde(skip)]
    pub candidate: Vec<u8>,
    candidate_disk_sha256: String,
    candidate_disk_size: usize,
    changed_bytes: usize,
    writes: Vec<PlannedWrite>,
    machine_code_verified: bool,
    entire_diff_verified: bool,
    installation_lifetime_verified: bool,
}

pub fn verify_with_menu(
    source: &[u8],
    page: &[u8],
    stream: u16,
    probe: Probe,
    menu: Option<crate::menu_hook::MenuHook>,
    text: Option<crate::text_install::TextInstallation>,
    additional: (Vec<ExpectedWrite>, Vec<CodeSource>),
) -> Result<Receipt, Box<dyn Error>> {
    if page.len() != 0x4000 {
        return Err("Storage page must be exactly 16KiB".into());
    }
    let mut entry = Assembler::new();
    entry.emit(Instruction::Jp(0x1fb0));
    let mut verifier = Verifier(vec![
        CodeSource {
            id: "loader-entry".into(),
            disk_offset: 0xb0378,
            program: entry.assemble(0x0478)?,
        },
        CodeSource {
            id: "loader-restore".into(),
            disk_offset: 0xb1eb0,
            program: loader_restore::assemble(0x1fb0, stream)?,
        },
    ]);
    if probe != Probe::Storage {
        use retro_z80::{Condition, StackRegister};
        if source.get(0x24518..0x2451b) != Some(&[0xc3, 0x85, 0x4c][..]) {
            return Err("Main-entry instruction changed".into());
        }
        let mut entry = Assembler::new();
        entry.emit(Instruction::Jp(0x7d00));
        let mut transition = Assembler::new();
        transition
            .emit(Instruction::Push(StackRegister::Af))
            .emit(Instruction::Call(0x7c00))
            .jr_cond(Condition::C, "failed")
            .emit(Instruction::Pop(StackRegister::Af))
            .emit(Instruction::Jp(0x4c85))
            .label("failed")
            .emit(Instruction::Pop(StackRegister::Af))
            .emit(Instruction::Jp(0xc000));
        for (id, disk_offset, program) in [
            ("font-loader", 0x27c00, font_loader::assemble(0x7c00)?),
            ("font-transition", 0x27d00, transition.assemble(0x7d00)?),
        ] {
            if source
                .get(disk_offset..disk_offset + program.bytes().len())
                .ok_or("Missing font loader placement")?
                .iter()
                .any(|&b| b != 0xe5)
                || program.bytes().len() > 0x100
            {
                return Err("Font loader exceeds inspected page-09 padding".into());
            }
            verifier.0.push(CodeSource {
                id: id.into(),
                disk_offset,
                program,
            });
        }
        verifier.0.push(CodeSource {
            id: "main-entry".into(),
            disk_offset: 0x24518,
            program: entry.assemble(0x4518)?,
        });
    }
    if matches!(
        probe,
        Probe::PageOneRenderer | Probe::MainMenus | Probe::MainText
    ) {
        verifier.0.extend(crate::glyph_hook::sources(
            source,
            text.as_ref().map_or_else(
                || menu.as_ref().map_or(256, |m| m.glyph_count),
                |t| t.glyph_count,
            ),
            matches!(probe, Probe::MainMenus | Probe::MainText),
        )?);
    }
    let mut writes = vec![ExpectedWrite {
        id: "page21-storage".into(),
        owner: "font-storage".into(),
        purpose: "Full original page compressed alongside font input".into(),
        offset: 0x84000,
        expected_original: source.get(0x84000..0x88000).ok_or("Missing page")?.to_vec(),
        replacement: page.to_vec(),
        intent: WriteIntent::Data,
    }];
    if let Some(menu) = menu {
        if probe != Probe::MainMenus {
            return Err("Menu requires page-01 renderer".into());
        }
        verifier.0.extend(menu.code);
        writes.push(menu.data);
    }
    if let Some(text) = text {
        if probe != Probe::MainText {
            return Err("Main text installation requires main-text probe".into());
        }
        verifier.0.extend(text.code);
        writes.extend(text.data);
    }
    writes.extend(additional.0);
    for code in &verifier.0 {
        writes.push(ExpectedWrite {
            id: code.id.clone(),
            owner: "font-storage".into(),
            purpose: format!("Typed code for {}", code.id),
            offset: code.disk_offset,
            expected_original: source
                .get(code.disk_offset..code.disk_offset + code.program.bytes().len())
                .ok_or("Missing loader region")?
                .to_vec(),
            replacement: code.program.bytes().to_vec(),
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: code.id.clone(),
                isa_profile_id: PROFILE_ID.into(),
            }),
        });
    }
    verifier.0.extend(additional.1);
    audit(source, writes, verifier)
}

pub fn audit(
    source: &[u8],
    mut writes: Vec<ExpectedWrite>,
    verifier: Verifier,
) -> Result<Receipt, Box<dyn Error>> {
    writes.sort_by_key(|w| w.offset);
    let mut plan = WritePlan::new();
    let mut cursor = 0;
    for write in &writes {
        if cursor < write.offset {
            plan = plan.region(ImageRegion {
                id: format!("protected-{cursor:x}"),
                range: cursor..write.offset,
                kind: RegionKind::Protected,
                reason: "Outside storage experiment".into(),
            });
        }
        cursor = write.offset + write.replacement.len();
        plan = plan
            .region(ImageRegion {
                id: write.id.clone(),
                range: write.offset..cursor,
                kind: match write.intent {
                    WriteIntent::Data => RegionKind::Data,
                    WriteIntent::Metadata => RegionKind::Metadata,
                    WriteIntent::MachineCode(_) => RegionKind::MachineCode,
                },
                reason: write.purpose.clone(),
            })
            .write(write.clone());
    }
    plan = plan.region(ImageRegion {
        id: "protected-tail".into(),
        range: cursor..source.len(),
        kind: RegionKind::Protected,
        reason: "Remaining original disk".into(),
    });
    let output = plan.apply(source, Some(&verifier))?;
    plan.audit(source, &output, Some(&verifier))?;
    Ok(Receipt {
        candidate_disk_sha256: sha256(&output),
        candidate_disk_size: output.len(),
        changed_bytes: source.iter().zip(&output).filter(|(a, b)| a != b).count(),
        writes: writes
            .into_iter()
            .map(|write| {
                let code = verifier.0.iter().find(|code| code.id == write.id);
                PlannedWrite {
                    id: write.id,
                    offset: write.offset,
                    expected_hex: hex::encode(write.expected_original),
                    replacement_hex: hex::encode(write.replacement),
                    assembly_source_id: code.map(|s| s.id.clone()),
                    cpu_origin: code.map(|s| s.program.origin()),
                }
            })
            .collect(),
        machine_code_verified: true,
        entire_diff_verified: true,
        installation_lifetime_verified: false,
        candidate: output,
    })
}
