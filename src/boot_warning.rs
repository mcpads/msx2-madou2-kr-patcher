//! Render the low-memory warning from ROM-resident pixels, with no extra RAM buffer.
use crate::{cutscene_install::Installation, main_font, media::sha256, z80_writes::CodeSource};
use expected_write::{ExpectedWrite, MachineCodeProvenance, WriteIntent};
use retro_z80::{Assembler, ByteOperand::A, Instruction::*, Register16};
use serde::Deserialize;
use std::error::Error;

#[derive(Deserialize)]
struct Draft {
    source_sha256: String,
    policy: String,
    consumer: String,
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    id: String,
    source: Source,
    text: String,
    review: String,
}
#[derive(Deserialize)]
struct Source {
    disk_offset: usize,
    raw_hex: String,
    raw_sha256: String,
    initial_xy: [u8; 2],
}

pub fn install(
    source: &[u8],
    draft: &[u8],
    bdf: &[u8],
    result: &mut Installation,
) -> Result<(), Box<dyn Error>> {
    let d: Draft = serde_json::from_slice(draft)?;
    if d.source_sha256 != sha256(source)
        || d.policy != "non-distribution-development"
        || d.consumer != "page08:453E"
        || d.entries.len() != 3
        || sha256(bdf) != "2a6fd090ac6d24f7392d6cc49db02ce54b9d1c01048bf8c97cbcdbc5a885cb15"
    {
        return Err("Boot warning input identity changed".into());
    }
    if source.get(0x2005f..0x20070)
        != Some(hex::decode("2a4afc1100dbb7ed52d2de403e0ecd104e")?.as_slice())
    {
        return Err("Boot memory branch changed".into());
    }
    let characters: Vec<_> = d.entries.iter().flat_map(|e| e.text.chars()).collect();
    let glyphs = main_font::read_glyphs(std::str::from_utf8(bdf)?, &characters)?;
    let mut bitmap = vec![0u8; 32 * 40];
    for (i, entry) in d.entries.iter().enumerate() {
        let address = [0x40a1u16, 0x40b0, 0x40c4][i];
        let site = 0x4070 + i * 12;
        let mut call = vec![0x21];
        call.extend(address.to_le_bytes());
        call.extend(hex::decode("3e0306001600cd3e45")?);
        let raw = hex::decode(&entry.source.raw_hex)?;
        let xy = [[10, 10], [12, 12], [12, 14]][i];
        if source.get(0x1c000 + site..0x1c000 + site + call.len()) != Some(call.as_slice())
            || entry.id != format!("boot-warning-{address:04x}")
            || entry.source.disk_offset != 0x1c000 + usize::from(address)
            || source.get(entry.source.disk_offset..entry.source.disk_offset + raw.len())
                != Some(raw.as_slice())
            || sha256(&raw) != entry.source.raw_sha256
            || raw.last() != Some(&0xff)
            || raw.get(..2) != Some(&xy)
            || entry.source.initial_xy != xy
            || entry.review != "needs_review"
            || entry.text.is_empty()
            || entry.text.chars().any(|c| c.is_control())
            || usize::from(xy[0]) * 4 + entry.text.chars().count() * 8 > 256
        {
            return Err("Invalid boot warning source or line layout".into());
        }
        if i == 2 && !entry.text.contains("CTRL") {
            return Err("Missing source CTRL key".into());
        }
        for (column, character) in entry.text.chars().enumerate() {
            let x = usize::from(xy[0]) * 4 + column * 8;
            for (row, bits) in glyphs[&character].iter().enumerate() {
                let y = (usize::from(xy[1]) - 10) * 8 + row;
                for bit in 0..8 {
                    if bits & (128 >> bit) != 0 {
                        bitmap[y * 32 + (x + bit) / 8] |= 128 >> ((x + bit) % 8);
                    }
                }
            }
        }
    }
    let mut pixels = (bitmap.len() as u16).to_le_bytes().to_vec();
    pixels.extend(bitmap);
    // 4914 expands directly into VRAM bank 4, at 12800; copy to visible y=80.
    let mut hook = Assembler::new();
    hook.emit(LdRRImm(Register16::Hl, 0x7300))
        .emit(LdRRImm(Register16::De, 0x2800))
        .emit(LdRImm(A, 0))
        .emit(Call(0x4914))
        .emit(Call(0x4c4d));
    let program = hook.assemble(0x7200)?;
    let rect_address = 0x7200 + u16::try_from(program.bytes().len())?;
    let rect = vec![0, 74, 64, 5, 0, 10, 0];
    let mut resume = Assembler::new();
    resume.emit(Call(0x501c)).emit(Jp(0x4094));
    let mut entry = Assembler::new();
    entry.emit(Jp(0x7200));
    for (id, program) in [
        ("boot-warning-entry", entry.assemble(0x4070)?),
        ("boot-warning-draw", program),
        ("boot-warning-return", resume.assemble(rect_address + 7)?),
    ] {
        let offset = 0x1c000 + usize::from(program.origin());
        if program.origin() >= 0x7200
            && source[offset..offset + program.bytes().len()]
                .iter()
                .any(|&b| b != 0xe5)
        {
            return Err("Boot warning code padding changed".into());
        }
        result.writes.push(ExpectedWrite {
            id: id.into(),
            owner: "boot-warning".into(),
            purpose: "Typed warning renderer".into(),
            offset,
            expected_original: source[offset..offset + program.bytes().len()].to_vec(),
            replacement: program.bytes().to_vec(),
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: id.into(),
                isa_profile_id: retro_z80::PROFILE_ID.into(),
            }),
        });
        result.machine_code.push(CodeSource {
            id: id.into(),
            disk_offset: offset,
            program,
        });
    }
    for (id, address, replacement, intent) in [
        ("boot-warning-bitmap", 0x7300, pixels, WriteIntent::Data),
        (
            "boot-warning-copy",
            rect_address,
            rect,
            WriteIntent::Metadata,
        ),
    ] {
        let offset = 0x1c000 + usize::from(address);
        let original = source
            .get(offset..offset + replacement.len())
            .ok_or("Warning exceeds source")?;
        if usize::from(address) + replacement.len() > 0x8000 || original.iter().any(|&b| b != 0xe5)
        {
            return Err("Boot warning data exceeds padding".into());
        }
        result.writes.push(ExpectedWrite {
            id: id.into(),
            owner: "boot-warning".into(),
            purpose: "Low-memory Korean warning".into(),
            offset,
            expected_original: original.to_vec(),
            replacement,
            intent,
        });
    }
    Ok(())
}
