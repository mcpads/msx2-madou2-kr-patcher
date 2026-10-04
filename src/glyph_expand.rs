//! Expand one packed 1bpp glyph into the original 4bpp slot 60.
//! DE=index; returns A=60/CY=0 or A=FF/CY=1 for an invalid index.
//! Preserves BC/DE/HL and index/alternate registers. Requires enabled interrupts
//! and an idle VDP command engine; the caller owns command synchronization.
use retro_z80::{
    AluOperation::*, AssembledProgram, Assembler, AssemblerError, ByteOperand::*, Condition,
    Instruction::*, Register16, RotateShiftOperation, StackRegister,
};

pub fn assemble(
    origin: u16,
    glyph_count: u16,
) -> Result<AssembledProgram, Box<dyn std::error::Error>> {
    if glyph_count == 0 || glyph_count > 896 {
        return Err("Packed glyph count outside VRAM capacity".into());
    }
    let mut code = Assembler::new();
    code.emit(Push(StackRegister::Bc))
        .emit(Push(StackRegister::De))
        .emit(Push(StackRegister::Hl))
        .emit(LdRR(H, D))
        .emit(LdRR(L, E))
        .emit(LdRRImm(Register16::Bc, glyph_count))
        .emit(AluR(Or, A))
        .emit(SbcHLRR(Register16::Bc))
        .jr_cond(Condition::Nc, "invalid")
        .emit(LdRR(H, D))
        .emit(LdRR(L, E));
    for _ in 0..3 {
        code.emit(AddHLRR(Register16::Hl));
    }
    code.emit(LdRR(A, H))
        .emit(AluImm(Cp, 0x0c))
        .jr_cond(Condition::C, "base")
        .emit(LdRRImm(Register16::Bc, 0x400))
        .emit(AddHLRR(Register16::Bc))
        .label("base")
        .emit(LdRRImm(Register16::Bc, 0x6000))
        .emit(AddHLRR(Register16::Bc))
        .emit(LdRRImm(Register16::De, 0x6c00))
        .emit(LdRImm(B, 8))
        .label("row")
        .call("read")
        .emit(IncRR(Register16::Hl))
        .emit(LdRR(C, A))
        .emit(Push(StackRegister::Bc))
        .emit(LdRImm(B, 4))
        .label("pair")
        .emit(AluR(Xor, A))
        .emit(Rot(RotateShiftOperation::Sla, C))
        .jr_cond(Condition::Nc, "low")
        .emit(AluImm(Or, 0xf0))
        .label("low")
        .emit(Rot(RotateShiftOperation::Sla, C))
        .jr_cond(Condition::Nc, "pixel")
        .emit(AluImm(Or, 0x0f))
        .label("pixel")
        .call("write")
        .emit(IncRR(Register16::De))
        .djnz("pair")
        .emit(Push(StackRegister::Hl))
        .emit(LdRRImm(Register16::Hl, 124))
        .emit(AddHLRR(Register16::De))
        .emit(ExDEHL)
        .emit(Pop(StackRegister::Hl))
        .emit(Pop(StackRegister::Bc))
        .djnz("row")
        .emit(Pop(StackRegister::Hl))
        .emit(Pop(StackRegister::De))
        .emit(Pop(StackRegister::Bc))
        .emit(LdRImm(A, 0x60))
        .emit(AluR(Or, A))
        .emit(Ret)
        .label("invalid")
        .emit(Pop(StackRegister::Hl))
        .emit(Pop(StackRegister::De))
        .emit(Pop(StackRegister::Bc))
        .emit(LdRImm(A, 0xff))
        .emit(Scf)
        .emit(Ret);
    emit_vram_io(&mut code);
    code.assemble(origin).map_err(|e: AssemblerError| e.into())
}

fn emit_vram_io(code: &mut Assembler) {
    code.label("read")
        .emit(Push(StackRegister::Bc))
        .emit(LdAAddr(0xc017))
        .emit(LdRR(C, A))
        .emit(IncR(C))
        .emit(LdRR(A, H))
        .emit(AluImm(And, 0xc0))
        .emit(Rlca)
        .emit(Rlca)
        .emit(Di)
        .emit(OutCR(A))
        .emit(LdRImm(A, 0x8e))
        .emit(OutCR(A))
        .emit(OutCR(L))
        .emit(LdRR(A, H))
        .emit(AluImm(And, 0x3f))
        .emit(OutCR(A))
        .emit(DecR(C))
        .emit(Nop)
        .emit(Nop)
        .emit(InRC(A))
        .emit(Ei)
        .emit(Pop(StackRegister::Bc))
        .emit(Ret)
        .label("write")
        .emit(Push(StackRegister::Bc))
        .emit(Push(StackRegister::Af))
        .emit(LdAAddr(0xc017))
        .emit(LdRR(C, A))
        .emit(IncR(C))
        .emit(LdRR(A, D))
        .emit(AluImm(And, 0xc0))
        .emit(Rlca)
        .emit(Rlca)
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
}
