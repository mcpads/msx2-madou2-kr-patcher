//! Translate the inspected single-cell money label without changing source glyphs.
use crate::{
    encode_main::EncodingReport, media::sha256, text_driver::PAGES, text_install::TextInstallation,
    z80_writes::CodeSource,
};
use retro_z80::{Assembler, Instruction::*, Register16, StackRegister};
use serde::Deserialize;
use std::error::Error;

#[derive(Deserialize)]
struct Label {
    source_sha256: String,
    policy: String,
    id: String,
    source_glyph: u8,
    source_text: String,
    text: String,
    review: String,
}

pub fn install(
    source: &[u8],
    draft: &[u8],
    encoding: &EncodingReport,
    result: &mut TextInstallation,
) -> Result<(), Box<dyn Error>> {
    let label: Label = serde_json::from_slice(draft)?;
    if label.source_sha256 != sha256(source)
        || label.policy != "non-distribution-development"
        || label.id != "money-label"
        || label.source_glyph != 0x65
        || label.source_text != "金"
        || label.review != "needs_review"
        || label.text.chars().count() != 1
    {
        return Err("Invalid source-linked single-cell money label".into());
    }
    let character = label.text.chars().next().ok_or("Empty money label")?;
    let index = encoding
        .characters()
        .position(|c| c == character)
        .ok_or("Money label is absent from the validated font repertoire")?;
    let glyph = u16::try_from(256 + index)?;
    for page in PAGES {
        let call = if page.number == 7 { 0x5551 } else { 0x4fa8 };
        let [low, high] = page.glyph.to_le_bytes();
        // Include the unchanged colon to bind the replacement to its actual consumer.
        let expected = [0x3e, 0x65, 0xcd, low, high, 0x3e, 0x1a, 0xcd, low, high];
        if source.get(page.disk(call)..page.disk(call) + expected.len()) != Some(&expected) {
            return Err("Money label/colon consumer changed".into());
        }
        let pool = result
            .data
            .iter()
            .find(|w| w.id == format!("page{}-local-text", page.number))
            .ok_or("Missing local text placement")?;
        let offset = pool.offset + pool.replacement.len();
        let origin = u16::try_from(offset - page.number * 0x4000 + 0x4000)?;
        let mut hook = Assembler::new();
        hook.emit(Push(StackRegister::Bc))
            .emit(Call(page.wait))
            .emit(LdRRImm(Register16::De, glyph))
            .emit(Call(0x7800))
            .emit(Pop(StackRegister::Bc))
            .emit(LdRRImm(Register16::De, 0x001b))
            .emit(Jp(page.glyph + 14));
        let program = hook.assemble(origin)?;
        let limit = if page.number == 7 {
            0x7800
        } else {
            crate::text_driver::MODE
        };
        if usize::from(origin) + program.bytes().len() > usize::from(limit)
            || source
                .get(offset..offset + program.bytes().len())
                .ok_or("Missing money-label placement")?
                .iter()
                .any(|&b| b != 0xe5)
        {
            return Err("Money label hook exceeds inspected local padding".into());
        }
        let mut entry = Assembler::new();
        entry.emit(Call(origin)).emit(Nop).emit(Nop);
        result.code.extend([
            CodeSource {
                id: format!("page{}-money-label-entry", page.number),
                disk_offset: page.disk(call),
                program: entry.assemble(call)?,
            },
            CodeSource {
                id: format!("page{}-money-label", page.number),
                disk_offset: offset,
                program,
            },
        ]);
    }
    Ok(())
}
