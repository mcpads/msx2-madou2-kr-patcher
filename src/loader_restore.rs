//! Wrap the immutable disk-reader tail, preserving its complete register result.
use crate::page_restore;
use retro_z80::{
    AluOperation::*, AssembledProgram, Assembler, AssemblerError, ByteOperand::*, Condition,
    Instruction::*, Register16, StackRegister,
};

pub fn assemble(origin: u16, stream: u16) -> Result<AssembledProgram, AssemblerError> {
    let mut code = Assembler::new();
    code.emit(Push(StackRegister::Af)) // Page ID before the overwritten multiplication.
        .emit(AluR(Add, A))
        .emit(AluR(Add, A))
        .emit(LdRR(L, A))
        .emit(Call(0x047b)) // Original tail; success is Z, not necessarily A == 0.
        .emit(ExSPHL) // Save returned HL; recover the page ID in H.
        .emit(Push(StackRegister::Af))
        .jr_cond(Condition::Nz, "return_reader")
        .emit(LdRR(A, H))
        .emit(AluImm(Cp, 0x21))
        .jr_cond(Condition::Nz, "return_reader")
        .emit(Push(StackRegister::Bc))
        .emit(Push(StackRegister::De))
        .emit(LdRRImm(Register16::Hl, stream))
        .emit(LdRRImm(Register16::De, 0x8000))
        .call("packet")
        .emit(Pop(StackRegister::De))
        .emit(Pop(StackRegister::Bc))
        .label("return_reader")
        .emit(Pop(StackRegister::Af))
        .emit(Pop(StackRegister::Hl))
        .emit(Ret);
    page_restore::emit(&mut code);
    code.assemble(origin)
}
