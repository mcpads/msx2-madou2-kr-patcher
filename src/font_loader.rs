//! Read packed page 21 without the ordinary restoration hook; stream its font
//! into lower VRAM 6000..7FFF. Caller supplies an already validated font page.
//! BC/DE/HL/IX/IY are preserved; AF reports success or read failure (CY=1).
//! Requires enabled interrupts, like the original per-byte VDP writer.
//! Overwrites the disk buffer; caller must reconcile page-cache state before use.
use retro_z80::{
    AluOperation::*, AssembledProgram, Assembler, AssemblerError, ByteOperand::*, Condition,
    IndexRegister, Instruction::*, Register16, StackRegister,
};

pub fn assemble(origin: u16) -> Result<AssembledProgram, AssemblerError> {
    let mut code = Assembler::new();
    code.emit(Push(StackRegister::Bc))
        .emit(Push(StackRegister::De))
        .emit(Push(StackRegister::Hl))
        .emit(PushIx(IndexRegister::Ix))
        .emit(PushIx(IndexRegister::Iy))
        .emit(Call(0xc00c)) // Original resident entry selects loader RAM at page 0.
        .emit(Call(0xc102)) // Save caller's timer hooks, install disk hooks.
        .emit(LdRImm(L, 0x84)) // Absolute page 21 * 4, before reader's remaining *8.
        .emit(Call(0x047b)) // Bypass page-restoration entry at 0478.
        .emit(Push(StackRegister::Af))
        .emit(Call(0x0398)) // Restore BIOS mapping, as original resident reader does.
        .emit(Call(0xc126)) // Restore all ten caller hook bytes at FD9A..FDA3.
        .emit(Pop(StackRegister::Af)) // Preserve reader Z across slot/hook helpers.
        .jr_cond(Condition::Nz, "read_error")
        .emit(LdRRImm(Register16::Hl, 0x8000))
        .emit(LdRRImm(Register16::De, 0x6000))
        .call("packet")
        .emit(AluR(Xor, A))
        .jr("done")
        .label("read_error")
        .emit(Scf)
        .label("done")
        .emit(PopIx(IndexRegister::Iy))
        .emit(PopIx(IndexRegister::Ix))
        .emit(Pop(StackRegister::Hl))
        .emit(Pop(StackRegister::De))
        .emit(Pop(StackRegister::Bc))
        .emit(Ei)
        .emit(Ret)
        .label("packet")
        .emit(LdRR(A, IndirectHl))
        .emit(IncRR(Register16::Hl))
        .emit(AluR(Or, A))
        .emit(RetCond(Condition::Z))
        .emit(LdRR(C, A))
        .emit(AluImm(And, 0x7f))
        .emit(LdRR(B, A))
        .emit(Bit(7, C))
        .jr_cond(Condition::Z, "literal")
        .emit(LdRR(A, IndirectHl))
        .emit(IncRR(Register16::Hl))
        .label("repeat")
        .call("write")
        .emit(IncRR(Register16::De))
        .djnz("repeat")
        .jr("packet")
        .label("literal")
        .emit(LdRR(A, IndirectHl))
        .emit(IncRR(Register16::Hl))
        .call("write")
        .emit(IncRR(Register16::De))
        .djnz("literal")
        .jr("packet")
        .label("write")
        .emit(Push(StackRegister::Bc))
        .emit(Push(StackRegister::Af))
        .emit(LdAAddr(0xc017))
        .emit(LdRR(C, A))
        .emit(IncR(C))
        .emit(LdRR(A, D))
        .emit(AluImm(And, 0xc0))
        .emit(Rlca)
        .emit(Rlca) // Original OR 04 selects upper 64KiB; intentionally omitted.
        .emit(Di)
        .emit(OutCR(A))
        .emit(LdRImm(A, 0x8e))
        .emit(OutCR(A))
        .emit(OutCR(E))
        .emit(LdRR(A, D))
        .emit(AluImm(And, 0x3f))
        .emit(AluImm(Or, 0x40))
        .emit(OutCR(A))
        .emit(Pop(StackRegister::Af))
        .emit(DecR(C))
        .emit(OutCR(A))
        .emit(Ei)
        .emit(Pop(StackRegister::Bc))
        .emit(Ret);
    code.assemble(origin)
}
