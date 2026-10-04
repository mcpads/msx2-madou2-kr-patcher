//! Runtime ending classification and particle selection from the validated glyph order.
use crate::{encode_main::EncodingReport, main_encoding::PARTICLES};
use retro_z80::{
    AluOperation::*, Assembler, ByteOperand::*, Condition, Instruction::*, Register16,
    RotateShiftOperation, StackRegister,
};
use serde::Serialize;
use std::{collections::BTreeMap, error::Error};

// Packed values: 0 keeps prior ending, 1 vowel, 2 consonant, 3 unknown.
fn ending(character: char) -> u8 {
    let code = character as u32;
    if (0xac00..=0xd7a3).contains(&code) {
        if (code - 0xac00).is_multiple_of(28) {
            1
        } else {
            2
        }
    } else if character.is_ascii_digit() {
        if "013678".contains(character) { 2 } else { 1 }
    } else if character.is_alphanumeric() {
        3
    } else {
        0
    }
}

#[derive(Serialize)]
pub struct PreparedParticles {
    classify_origin: u16,
    select_origin: u16,
    table_origin: u16,
    pairs_origin: u16,
    glyph_count: usize,
    classify_hex: String,
    select_hex: String,
    table_hex: String,
    pairs_hex: String,
    pub game_installed: bool,
}

pub fn prepare(
    encoding: &EncodingReport,
    origin: u16,
) -> Result<PreparedParticles, Box<dyn Error>> {
    let glyph_count = 256 + encoding.characters().count();
    let (table, pairs) = tables(encoding)?;
    let select_origin = origin
        .checked_add(0x80)
        .ok_or("Particle code placement overflow")?;
    let table_origin = origin
        .checked_add(0x100)
        .ok_or("Particle table placement overflow")?;
    let pairs_origin = table_origin
        .checked_add(u16::try_from(table.len())?)
        .ok_or("Particle pairs placement overflow")?;
    pairs_origin
        .checked_add(u16::try_from(pairs.len())?)
        .ok_or("Particle data placement overflow")?;
    let classify = classify(origin, table_origin, u16::try_from(glyph_count)?)?;
    let select = select(select_origin, pairs_origin)?;
    if classify.bytes().len() > 0x80 || select.bytes().len() > 0x80 {
        return Err("Particle code overlaps reserved range".into());
    }
    Ok(PreparedParticles {
        classify_origin: origin,
        select_origin,
        table_origin,
        pairs_origin,
        glyph_count,
        classify_hex: hex::encode(classify.bytes()),
        select_hex: hex::encode(select.bytes()),
        table_hex: hex::encode(table),
        pairs_hex: hex::encode(pairs),
        game_installed: false,
    })
}

pub fn tables(encoding: &EncodingReport) -> Result<(Vec<u8>, Vec<u8>), Box<dyn Error>> {
    let characters: Vec<_> = encoding.characters().collect();
    let glyph_count = 256 + characters.len();
    let mut classes = vec![3; glyph_count];
    for character in ' '..='~' {
        classes[character as usize - 32] = ending(character);
    }
    for (index, &character) in characters.iter().enumerate() {
        classes[index + 256] = ending(character);
    }
    let mut table = vec![0; glyph_count.div_ceil(4)];
    for (index, &class) in classes.iter().enumerate() {
        table[index / 4] |= class << ((index % 4) * 2);
    }
    let indices: BTreeMap<_, _> = characters
        .iter()
        .enumerate()
        .map(|(i, &c)| (c, (i + 256) as u16))
        .collect();
    let mut pairs = Vec::new();
    for (_, pair) in PARTICLES {
        for character in pair.chars() {
            pairs.extend(
                indices
                    .get(&character)
                    .ok_or("Particle glyph missing")?
                    .to_le_bytes(),
            );
        }
    }
    Ok((table, pairs))
}

/// A=prior ending (1/2/3), DE=glyph. Returns updated A, preserves BC/DE/HL.
pub fn classify(
    origin: u16,
    table: u16,
    count: u16,
) -> Result<retro_z80::AssembledProgram, Box<dyn Error>> {
    let mut c = Assembler::new();
    c.emit(Push(StackRegister::Bc))
        .emit(Push(StackRegister::De))
        .emit(Push(StackRegister::Hl))
        .emit(LdRR(C, A))
        .emit(LdRRImm(Register16::Hl, count))
        .emit(AluR(Or, A))
        .emit(SbcHLRR(Register16::De))
        .jr_cond(Condition::C, "invalid")
        .jr_cond(Condition::Z, "invalid")
        .emit(LdRR(A, E))
        .emit(AluImm(And, 3))
        .emit(Rlca)
        .emit(LdRR(B, A));
    for _ in 0..2 {
        c.emit(Rot(RotateShiftOperation::Srl, D))
            .emit(Rot(RotateShiftOperation::Rr, E));
    }
    c.emit(LdRRImm(Register16::Hl, table))
        .emit(AddHLRR(Register16::De))
        .emit(LdRR(A, IndirectHl))
        .emit(IncR(B))
        .jr("shift_test")
        .label("shift")
        .emit(Rrca)
        .label("shift_test")
        .djnz("shift")
        .emit(AluImm(And, 3))
        .jr_cond(Condition::Nz, "result")
        .emit(LdRR(A, C))
        .label("result")
        .emit(AluR(Or, A))
        .jr("return")
        .label("invalid")
        .emit(LdRImm(A, 255))
        .emit(Scf)
        .label("return")
        .emit(Pop(StackRegister::Hl))
        .emit(Pop(StackRegister::De))
        .emit(Pop(StackRegister::Bc))
        .emit(Ret);
    Ok(c.assemble(origin)?)
}

/// A=ending (1/2), E=selector (1..4). Returns DE=glyph, CY=0; unknown/error CY=1.
pub fn select(origin: u16, pairs: u16) -> Result<retro_z80::AssembledProgram, Box<dyn Error>> {
    let mut c = Assembler::new();
    c.emit(Push(StackRegister::Bc))
        .emit(Push(StackRegister::Hl))
        .emit(AluImm(Cp, 1))
        .jr_cond(Condition::C, "invalid")
        .emit(AluImm(Cp, 3))
        .jr_cond(Condition::Nc, "invalid")
        .emit(LdRR(C, A))
        .emit(LdRR(A, E))
        .emit(DecR(A))
        .emit(AluImm(Cp, 4))
        .jr_cond(Condition::Nc, "invalid")
        .emit(Rlca)
        .emit(Rlca)
        .emit(LdRR(L, A))
        .emit(LdRImm(H, 0))
        .emit(LdRR(A, C))
        .emit(AluImm(Cp, 2))
        .jr_cond(Condition::Z, "lookup")
        .emit(IncRR(Register16::Hl))
        .emit(IncRR(Register16::Hl))
        .label("lookup")
        .emit(LdRRImm(Register16::Bc, pairs))
        .emit(AddHLRR(Register16::Bc))
        .emit(LdRR(E, IndirectHl))
        .emit(IncRR(Register16::Hl))
        .emit(LdRR(D, IndirectHl))
        .emit(AluR(Xor, A))
        .jr("return")
        .label("invalid")
        .emit(LdRImm(A, 255))
        .emit(Scf)
        .label("return")
        .emit(Pop(StackRegister::Hl))
        .emit(Pop(StackRegister::Bc))
        .emit(Ret);
    Ok(c.assemble(origin)?)
}

#[cfg(test)]
#[path = "particles_tests.rs"]
mod tests;
