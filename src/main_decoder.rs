//! Candidate Korean token decoder.
use retro_z80::{
    AluOperation::*, AssembledProgram, Assembler, ByteOperand::*, Condition, Instruction::*,
    Register16, StackRegister,
};

pub fn assemble(
    origin: u16,
    glyph_limit: u16,
) -> Result<AssembledProgram, Box<dyn std::error::Error>> {
    assemble_with_reader(origin, glyph_limit, None)
}

/// The reader returns A=(HL) without advancing HL and preserves BC/DE/HL,
/// IX/IY and alternate registers. A game reader may restore enabled interrupts.
pub fn assemble_with_reader(
    origin: u16,
    glyph_limit: u16,
    reader: Option<u16>,
) -> Result<AssembledProgram, Box<dyn std::error::Error>> {
    if !(256..=896).contains(&glyph_limit) {
        return Err("Invalid font capacity".into());
    }
    let mut code = Assembler::new();
    code.label("decode_next");
    code.call("read_byte");
    code.jp_cond(Condition::C, "invalid");
    code.emit(AluImm(Cp, 255));
    code.jp_cond(Condition::Z, "end_token");
    code.emit(AluImm(Cp, 36));
    code.jp_cond(Condition::Z, "command");
    code.emit(AluImm(Cp, 4));
    code.jr_cond(Condition::Z, "particle");
    code.emit(AluR(Or, A));
    code.jp_cond(Condition::Z, "invalid");
    code.emit(AluImm(Cp, 4));
    code.jr_cond(Condition::C, "extended");
    code.emit(AluImm(Cp, 32));
    code.jr_cond(Condition::C, "single_low");
    code.emit(AluImm(Cp, 127));
    code.jr_cond(Condition::C, "ascii");
    code.emit(AluImm(Cp, 161));
    code.jr_cond(Condition::C, "single_middle");
    code.emit(AluImm(Cp, 224));
    code.jp_cond(Condition::C, "invalid");
    code.emit(AluImm(Sub, 224));
    code.emit(LdRRImm(Register16::De, 317));
    code.jr("single");
    code.label("single_middle");
    code.emit(AluImm(Sub, 127));
    code.emit(LdRRImm(Register16::De, 283));
    code.jr("single");
    code.label("single_low");
    code.emit(AluImm(Sub, 5));
    code.emit(LdRRImm(Register16::De, 256));
    code.label("single");
    code.emit(AluR(Add, E));
    code.emit(LdRR(E, A));
    code.jr_cond(Condition::Nc, "check_glyph");
    code.emit(IncR(D));
    code.jr("check_glyph");
    code.label("extended");
    code.emit(LdRR(D, A));
    code.call("read_byte");
    code.jp_cond(Condition::C, "invalid");
    code.emit(LdRR(E, A));
    code.emit(Push(StackRegister::Hl));
    code.emit(LdRRImm(Register16::Hl, 92));
    code.emit(AddHLRR(Register16::De));
    code.emit(ExDEHL);
    code.emit(Pop(StackRegister::Hl));
    code.label("check_glyph");
    code.emit(Push(StackRegister::Hl));
    code.emit(LdRRImm(Register16::Hl, glyph_limit));
    code.emit(AluR(Or, A));
    code.emit(SbcHLRR(Register16::De));
    code.emit(Pop(StackRegister::Hl));
    code.jp_cond(Condition::C, "invalid");
    code.jp_cond(Condition::Z, "invalid");
    code.emit(AluR(Xor, A));
    code.emit(Ret);
    code.label("ascii");
    code.emit(AluImm(Sub, 32));
    code.emit(LdRR(E, A));
    code.emit(LdRImm(D, 0));
    code.emit(AluR(Xor, A));
    code.emit(Ret);
    code.label("particle");
    code.call("read_byte");
    code.jp_cond(Condition::C, "invalid");
    code.emit(AluR(Or, A));
    code.jp_cond(Condition::Z, "invalid");
    code.emit(AluImm(Cp, 5));
    code.jp_cond(Condition::Nc, "invalid");
    code.emit(LdRR(E, A));
    code.emit(LdRImm(D, 0));
    code.emit(LdRImm(A, 2));
    code.emit(Ret);
    code.label("command");
    code.call("read_byte");
    code.jp_cond(Condition::C, "invalid");
    code.emit(AluImm(Cp, 36));
    code.jr_cond(Condition::Z, "ascii");
    code.emit(LdRR(E, A));
    code.emit(LdRImm(D, 0));
    code.emit(AluImm(Cp, b'w'));
    code.jr_cond(Condition::Z, "command_argument");
    code.emit(AluImm(Cp, b't'));
    code.jr_cond(Condition::Z, "command_argument");
    code.emit(AluImm(Cp, b'r'));
    code.jr_cond(Condition::Z, "command_done");
    code.emit(AluImm(Cp, b'c'));
    code.jr_cond(Condition::Z, "command_done");
    code.emit(AluImm(Cp, b'k'));
    code.jr_cond(Condition::Z, "command_done");
    code.emit(AluImm(Cp, b'@'));
    code.jp_cond(Condition::Nz, "invalid");
    code.label("command_done");
    code.emit(LdRImm(A, 1));
    code.emit(Ret);
    code.label("command_argument");
    code.call("read_byte");
    code.jp_cond(Condition::C, "invalid");
    code.emit(LdRR(D, A));
    code.jr("command_done");
    code.label("end_token");
    code.emit(LdRImm(A, 3));
    code.emit(Ret);
    code.label("invalid");
    code.emit(LdRImm(A, 255));
    code.emit(Ret);
    code.label("read_byte");
    code.emit(LdRR(A, B));
    code.emit(AluR(Or, C));
    code.jr_cond(Condition::Z, "read_empty");
    if let Some(reader) = reader {
        code.emit(Call(reader));
    } else {
        code.emit(LdRR(A, IndirectHl));
    }
    code.emit(IncRR(Register16::Hl));
    code.emit(DecRR(Register16::Bc));
    code.emit(AluR(Or, A));
    code.emit(Ret);
    code.label("read_empty");
    code.emit(Scf);
    code.emit(Ret);
    Ok(code.assemble(origin)?)
}
