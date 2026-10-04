//! Forward in-place restoration of a validated 16KiB page.
//! HL points to the packet stream ending at C000; DE starts at 8000.
//! The builder must prove packet bounds and no overwrite of unread input.
//! There is no terminator byte: stop when the output reaches C000.
use retro_z80::{
    AluOperation::*, AssembledProgram, Assembler, AssemblerError, ByteOperand::*, Condition,
    Instruction::*, Register16,
};

pub fn assemble(origin: u16) -> Result<AssembledProgram, AssemblerError> {
    let mut code = Assembler::new();
    emit(&mut code);
    code.assemble(origin)
}

pub fn emit(code: &mut Assembler) {
    code.label("packet")
        .emit(LdRR(A, D))
        .emit(AluImm(Cp, 0xc0))
        .emit(RetCond(Condition::Z))
        .emit(LdRR(A, IndirectHl))
        .emit(IncRR(Register16::Hl))
        .emit(AluR(Or, A))
        .jr_cond(Condition::Z, "invalid")
        .emit(LdRR(C, A))
        .emit(AluImm(And, 0x7f))
        .emit(LdRR(B, A))
        .emit(Bit(7, C))
        .jr_cond(Condition::Z, "literal")
        .emit(LdRR(A, IndirectHl))
        .emit(IncRR(Register16::Hl))
        .label("repeat")
        .emit(LdDEA)
        .emit(IncRR(Register16::De))
        .djnz("repeat")
        .jr("packet")
        .label("literal")
        .emit(LdRR(A, IndirectHl))
        .emit(IncRR(Register16::Hl))
        .emit(LdDEA)
        .emit(IncRR(Register16::De))
        .djnz("literal")
        .jr("packet")
        .label("invalid")
        .emit(Scf)
        .emit(Ret);
}
