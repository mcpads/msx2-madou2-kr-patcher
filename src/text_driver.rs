//! One-token main text driver. Keeps the original window, wait and input handlers.
use retro_z80::{
    AluOperation::*, AssembledProgram, Assembler, ByteOperand::*, Condition, Instruction::*,
    Register16, StackRegister,
};
use std::error::Error;

pub const TICK: u16 = 0x7920;
pub const DECODER: u16 = 0x7a00;
pub const INIT: u16 = 0x7ae0;
pub const DRAW: u16 = 0x7b80;
pub const CLASSIFY: u16 = 0x7be0;
pub const SELECT: u16 = 0x7c20;
pub const TABLE: u16 = 0x7c60;
pub const MENU: u16 = 0x7d40;
pub const TEXT: u16 = 0x7da0;
pub const MODE: u16 = 0x7ff8;
pub const REMAINING: u16 = 0x7ff9;
pub const ENDING: u16 = 0x7ffb;

#[derive(Clone, Copy)]
pub struct Page {
    pub number: usize,
    pub common: u16,
    pub glyph: u16,
    pub wait: u16,
    pub menu: u16,
}
pub const PAGES: [Page; 7] = [
    Page {
        number: 1,
        common: 0x69cd,
        glyph: 0x662b,
        wait: 0x6d54,
        menu: 0x4dae,
    },
    Page {
        number: 2,
        common: 0x6ad8,
        glyph: 0x6736,
        wait: 0x6e5f,
        menu: 0x4dae,
    },
    Page {
        number: 3,
        common: 0x69ca,
        glyph: 0x6628,
        wait: 0x6d51,
        menu: 0x4dae,
    },
    Page {
        number: 4,
        common: 0x6a0a,
        glyph: 0x6668,
        wait: 0x6d91,
        menu: 0x4dae,
    },
    Page {
        number: 5,
        common: 0x69df,
        glyph: 0x663d,
        wait: 0x6d66,
        menu: 0x4dae,
    },
    Page {
        number: 6,
        common: 0x69e9,
        glyph: 0x6647,
        wait: 0x6d70,
        menu: 0x4dae,
    },
    Page {
        number: 7,
        common: 0x5f33,
        glyph: 0x5abf,
        wait: 0x62ba,
        menu: 0x5357,
    },
];

impl Page {
    pub fn disk(self, address: u16) -> usize {
        (self.number - 1) * 0x4000 + usize::from(address)
    }
    pub fn reader(self) -> u16 {
        self.common + 0x71
    }
    pub fn tick(self) -> u16 {
        self.common - 0x23d
    }
    pub fn init(self) -> u16 {
        self.common - 0x280
    }
    pub fn newline(self) -> u16 {
        self.common - 0x126
    }
}

pub fn tick(page: Page) -> Result<AssembledProgram, Box<dyn Error>> {
    let mut c = Assembler::new();
    c.emit(LdAAddr(MODE))
        .emit(AluR(Or, A))
        .jr_cond(Condition::Nz, "translated")
        .emit(LdRImm(A, 3))
        .emit(LdAddrA(ENDING))
        .emit(LdAAddr(0xc99a))
        .emit(Jp(page.tick() + 3))
        .label("translated")
        .emit(LdAAddr(0xc99a))
        .emit(AluR(Or, A))
        .emit(RetCond(Condition::Z))
        .emit(LdAAddr(0xc998))
        .emit(AluR(Or, A))
        .jr_cond(Condition::Z, "decode")
        .emit(DecR(A))
        .emit(LdAddrA(0xc998))
        .emit(Ret)
        .label("decode")
        .emit(LdHLAddr(0xc990))
        .emit(LdRRAddr(Register16::Bc, REMAINING))
        .emit(Call(DECODER))
        .emit(LdAddrHL(0xc990))
        .emit(LdAddrRR(REMAINING, Register16::Bc))
        .emit(AluR(Or, A))
        .jr_cond(Condition::Z, "glyph")
        .emit(AluImm(Cp, 2))
        .jr_cond(Condition::Z, "particle")
        .emit(AluImm(Cp, 3))
        .jr_cond(Condition::Z, "end")
        .emit(AluImm(Cp, 1))
        .emit(JpCond(Condition::Nz, 0xc000))
        .emit(LdRR(A, E))
        .emit(AluImm(Cp, b'w'))
        .jr_cond(Condition::Z, "wait")
        .emit(AluImm(Cp, b't'))
        .jr_cond(Condition::Z, "speed")
        .emit(AluImm(Cp, b'r'))
        .emit(JpCond(Condition::Z, page.newline()))
        .emit(AluImm(Cp, b'c'))
        .emit(JpCond(Condition::Z, page.common - 0xd8))
        .emit(AluImm(Cp, b'k'))
        .emit(JpCond(Condition::Z, page.common + 0xaed))
        .emit(Jp(0xc000))
        .label("wait")
        .emit(LdRR(A, D))
        .emit(LdAddrA(0xc998))
        .emit(Ret)
        .label("speed")
        .emit(LdRR(A, D))
        .emit(LdAddrA(0xc999))
        .emit(Ret)
        .label("end")
        .emit(AluR(Xor, A))
        .emit(LdAddrA(0xc99a))
        .emit(LdAddrA(0xc9ab))
        .emit(LdAddrA(0xc9ac))
        .emit(Ret)
        .label("particle")
        .emit(LdAAddr(ENDING))
        .emit(Call(SELECT))
        .emit(JpCond(Condition::C, 0xc000))
        .label("glyph")
        .emit(LdAAddr(ENDING))
        .emit(Call(CLASSIFY))
        .emit(JpCond(Condition::C, 0xc000))
        .emit(LdAddrA(ENDING))
        .emit(Jp(DRAW));
    Ok(c.assemble(TICK)?)
}

pub fn init(
    page: Page,
    common_start: u16,
    common_end: u16,
    banked_end: u16,
) -> Result<AssembledProgram, Box<dyn Error>> {
    let mut c = Assembler::new();
    c.emit(Push(StackRegister::Af))
        .emit(Push(StackRegister::Bc))
        .emit(Push(StackRegister::De))
        .emit(Push(StackRegister::Hl))
        .emit(AluR(Xor, A))
        .emit(LdAddrA(MODE))
        .emit(LdAAddr(0xc9ab))
        .emit(AluR(Or, A))
        .jr_cond(Condition::Z, "common")
        .emit(LdRRImm(Register16::De, banked_end))
        .jr("bounded")
        .label("common")
        .emit(Push(StackRegister::Hl))
        .emit(LdRRImm(Register16::De, common_start))
        .emit(AluR(Or, A))
        .emit(SbcHLRR(Register16::De))
        .emit(Pop(StackRegister::Hl))
        .jr_cond(Condition::C, "local")
        .emit(Push(StackRegister::Hl))
        .emit(LdRRImm(Register16::De, common_end))
        .emit(AluR(Or, A))
        .emit(SbcHLRR(Register16::De))
        .emit(Pop(StackRegister::Hl))
        .jr_cond(Condition::Nc, "local")
        .emit(LdRRImm(Register16::De, common_end))
        .jr("bounded")
        .label("local")
        .emit(Call(page.reader()))
        .emit(AluR(Or, A))
        .jr_cond(Condition::Nz, "store")
        .emit(IncRR(Register16::Hl))
        .emit(LdRR(C, IndirectHl))
        .emit(IncRR(Register16::Hl))
        .emit(LdRR(B, IndirectHl))
        .emit(IncRR(Register16::Hl))
        .jr("enable")
        .label("bounded")
        .emit(Push(StackRegister::Hl))
        .emit(ExDEHL)
        .emit(AluR(Or, A))
        .emit(SbcHLRR(Register16::De))
        .emit(JpCond(Condition::C, 0xc000))
        .emit(LdRR(B, H))
        .emit(LdRR(C, L))
        .emit(Pop(StackRegister::Hl))
        .label("enable")
        .emit(LdAddrRR(REMAINING, Register16::Bc))
        .emit(LdRImm(A, 1))
        .emit(LdAddrA(MODE))
        .label("store")
        .emit(LdAddrHL(0xc990))
        .emit(Pop(StackRegister::Hl))
        .emit(Pop(StackRegister::De))
        .emit(Pop(StackRegister::Bc))
        .emit(Pop(StackRegister::Af))
        .emit(Ret);
    Ok(c.assemble(INIT)?)
}

pub fn draw(page: Page) -> Result<AssembledProgram, Box<dyn Error>> {
    let mut c = Assembler::new();
    c.emit(Push(StackRegister::De))
        .emit(LdAAddr(0xc996))
        .emit(LdRR(B, A))
        .emit(LdAAddr(0xc992))
        .emit(AluR(Cp, B))
        .jr_cond(Condition::C, "position")
        // The original window reserves its inclusive rightmost cell for hanging
        // punctuation. Keep a sentence-ending period with the preceding line;
        // all other glyphs still wrap before entering that cell.
        .jr_cond(Condition::Nz, "newline")
        .emit(LdRR(A, D))
        .emit(AluR(Or, A))
        .jr_cond(Condition::Nz, "newline")
        .emit(LdRR(A, E))
        .emit(AluImm(Cp, b'.' - b' '))
        .jr_cond(Condition::Z, "position")
        .label("newline")
        .emit(Call(page.newline()))
        .label("position")
        .emit(LdAAddr(0xc992))
        .emit(Rlca)
        .emit(LdAddrA(0xc850))
        .emit(LdAAddr(0xc993))
        .emit(LdAddrA(0xc851))
        .emit(LdRImm(A, 8))
        .emit(LdAddrA(0xc856))
        .emit(LdRImm(A, 1))
        .emit(LdAddrA(0xc855))
        .emit(Call(page.wait))
        .emit(Pop(StackRegister::De))
        .emit(Call(0x7800))
        .emit(JpCond(Condition::C, 0xc000))
        .emit(LdRRImm(Register16::De, 0x001b))
        .emit(Call(page.glyph + 15))
        .emit(Call(page.common - 0x148))
        .emit(Jp(page.common - 0x137));
    Ok(c.assemble(DRAW)?)
}

pub fn menu(page: Page, banked_end: u16) -> Result<AssembledProgram, Box<dyn Error>> {
    let mut c = Assembler::new();
    c.emit(LdAAddr(0xc9ab))
        .emit(Push(StackRegister::Af))
        .emit(Call(page.common + 0x58))
        .emit(Push(StackRegister::Hl))
        .emit(LdRRImm(Register16::De, banked_end))
        .emit(ExDEHL)
        .emit(AluR(Or, A))
        .emit(SbcHLRR(Register16::De))
        .emit(JpCond(Condition::C, 0xc000))
        .emit(LdRR(B, H))
        .emit(LdRR(C, L))
        .emit(Pop(StackRegister::Hl))
        .label("next")
        .emit(Call(DECODER))
        .emit(AluR(Or, A))
        .jr_cond(Condition::Z, "draw")
        .emit(AluImm(Cp, 3))
        .emit(JpCond(Condition::Nz, 0xc000))
        .emit(Pop(StackRegister::Af))
        .emit(LdAddrA(0xc9ab))
        .emit(Jp(page.menu + 0x55))
        .label("draw")
        .emit(Push(StackRegister::Bc))
        .emit(Push(StackRegister::De))
        .emit(Call(page.wait))
        .emit(Pop(StackRegister::De))
        .emit(Call(0x7800))
        .emit(Pop(StackRegister::Bc))
        .emit(JpCond(Condition::C, 0xc000))
        .emit(LdRRImm(Register16::De, 0x001b))
        .emit(Call(page.glyph + 15))
        .jr("next");
    Ok(c.assemble(MENU)?)
}
