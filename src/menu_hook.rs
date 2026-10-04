//! Page-01 name consumer: retain ID resolution and layout, decode local Korean names.
use crate::{
    encode_main::EncodingReport, main_decoder, menu_draft, text_references, z80_writes::CodeSource,
};
use expected_write::{ExpectedWrite, WriteIntent};
use retro_z80::{
    AluOperation::*, Assembler, ByteOperand::*, Condition, Instruction::*, Register16,
    StackRegister,
};
use std::{collections::BTreeMap, error::Error};

pub struct MenuHook {
    pub code: Vec<CodeSource>,
    pub data: ExpectedWrite,
    pub glyph_count: u16,
    pub names: usize,
}

pub fn prepare(source: &[u8], encoding: &EncodingReport) -> Result<MenuHook, Box<dyn Error>> {
    let names = menu_draft::source_names(source)?;
    let references = text_references::extract(source)?;
    let offsets: BTreeMap<_, _> = references
        .iter()
        .map(|r| (r.id.as_str(), r.offset))
        .collect();
    let entries: BTreeMap<_, _> = encoding
        .entries
        .iter()
        .map(|e| (e.source_offset, e))
        .collect();
    let mut strings = BTreeMap::new();
    for name in &names {
        let index: usize = name
            .strip_prefix("banked-")
            .ok_or("Invalid name ID")?
            .parse()?;
        let offset = offsets.get(name.as_str()).ok_or("Missing name reference")?;
        let entry = entries.get(offset).ok_or("Missing encoded name")?;
        strings.insert(index, hex::decode(&entry.encoded_hex)?);
    }
    let data = pack_names(&strings)?;
    let glyph_count = u16::try_from(256 + encoding.characters().count())?;
    if source.get(0x4dae..0x4db2) != Some(&[0x78, 0x21, 0x00, 0x30][..])
        || source.get(0x4e03..0x4e11)
            != Some(hex::decode("3aecc93c32ecc93e0132eec9e1c1")?.as_slice())
    {
        return Err("Menu consumer entry or cleanup changed".into());
    }
    let mut entry = Assembler::new();
    entry.emit(Jp(0x7b00)).emit(Nop);
    let code = vec![
        CodeSource {
            id: "page01-menu-entry".into(),
            disk_offset: 0x4dae,
            program: entry.assemble(0x4dae)?,
        },
        CodeSource {
            id: "page01-menu-decoder".into(),
            disk_offset: 0x7a00,
            program: main_decoder::assemble(0x7a00, glyph_count)?,
        },
        CodeSource {
            id: "page01-menu-consumer".into(),
            disk_offset: 0x7b00,
            program: assemble(strings.last_key_value().ok_or("Empty names")?.0 + 1)?,
        },
    ];
    for block in &code[1..] {
        if block.program.bytes().len() > 256 {
            return Err("Menu code exceeds reserved block".into());
        }
        padding(source, block.disk_offset, block.program.bytes().len())?;
    }
    padding(source, 0x7c00, data.len())?;
    Ok(MenuHook {
        code,
        glyph_count,
        names: names.len(),
        data: ExpectedWrite {
            id: "page01-menu-names".into(),
            owner: "main-menu".into(),
            purpose: "Validated Korean name pointers, lengths and encoded text".into(),
            offset: 0x7c00,
            expected_original: source[0x7c00..0x7c00 + data.len()].to_vec(),
            replacement: data,
            intent: WriteIntent::Data,
        },
    })
}

fn padding(source: &[u8], offset: usize, length: usize) -> Result<(), Box<dyn Error>> {
    if offset + length > 0x8000
        || source
            .get(offset..offset + length)
            .ok_or("Missing menu placement")?
            .iter()
            .any(|&b| b != 0xe5)
    {
        return Err("Menu placement outside inspected padding".into());
    }
    Ok(())
}

fn pack_names(names: &BTreeMap<usize, Vec<u8>>) -> Result<Vec<u8>, Box<dyn Error>> {
    let count = names.last_key_value().ok_or("Empty names")?.0 + 1;
    if count > 256 {
        return Err("Name index exceeds byte range".into());
    }
    let mut data = vec![0; count * 4];
    for (&index, bytes) in names {
        if bytes.is_empty() || bytes.last() != Some(&255) {
            return Err("Unterminated name".into());
        }
        let address = u16::try_from(0x7c00 + data.len())?;
        let length = u16::try_from(bytes.len())?;
        data[index * 4..index * 4 + 2].copy_from_slice(&address.to_le_bytes());
        data[index * 4 + 2..index * 4 + 4].copy_from_slice(&length.to_le_bytes());
        data.extend(bytes);
    }
    if data.len() > 0x400 {
        return Err("Menu table and text exceed page padding".into());
    }
    Ok(data)
}

fn assemble(count: usize) -> Result<retro_z80::AssembledProgram, Box<dyn Error>> {
    let mut code = Assembler::new();
    code.emit(LdRR(A, B))
        .emit(AluImm(Cp, u8::try_from(count)?))
        .jr_cond(Condition::Nc, "fallback")
        .emit(LdRR(L, A))
        .emit(LdRImm(H, 0))
        .emit(AddHLRR(Register16::Hl))
        .emit(AddHLRR(Register16::Hl))
        .emit(LdRRImm(Register16::De, 0x7c00))
        .emit(AddHLRR(Register16::De))
        .emit(LdRR(E, IndirectHl))
        .emit(IncRR(Register16::Hl))
        .emit(LdRR(D, IndirectHl))
        .emit(LdRR(A, D))
        .emit(AluR(Or, E))
        .jr_cond(Condition::Z, "fallback")
        .emit(IncRR(Register16::Hl))
        .emit(LdRR(C, IndirectHl))
        .emit(IncRR(Register16::Hl))
        .emit(LdRR(B, IndirectHl))
        .emit(ExDEHL)
        .label("next")
        .emit(Call(0x7a00))
        .emit(AluR(Or, A))
        .jr_cond(Condition::Z, "draw")
        .emit(AluImm(Cp, 3))
        .emit(JpCond(Condition::Z, 0x4e03))
        .emit(Jp(0xc000))
        .label("draw")
        .emit(Push(StackRegister::Bc))
        .emit(Push(StackRegister::De))
        .emit(Call(0x6d54))
        .emit(Pop(StackRegister::De))
        .emit(Call(0x7800))
        .emit(Pop(StackRegister::Bc))
        .emit(JpCond(Condition::C, 0xc000))
        .emit(LdRRImm(Register16::De, 0x001b))
        .emit(Call(0x663a))
        .jr("next")
        .label("fallback")
        .emit(LdRR(A, B))
        .emit(LdRRImm(Register16::Hl, 0x3000))
        .emit(Jp(0x4db2));
    Ok(code.assemble(0x7b00)?)
}

#[cfg(test)]
#[path = "menu_hook_tests.rs"]
mod tests;
