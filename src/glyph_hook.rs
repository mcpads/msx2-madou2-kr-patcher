//! Adapters for the inspected main-game pages' original one-byte glyph calls.
use crate::{glyph_expand, z80_writes::CodeSource};
use retro_z80::{Assembler, ByteOperand::*, Instruction::*, Register16, StackRegister};
use std::error::Error;

struct Page {
    number: usize,
    entry: u16,
    wait: u16,
    ids: [&'static str; 3],
}

const PAGES: [Page; 7] = [
    Page {
        number: 1,
        entry: 0x662b,
        wait: 0x6d54,
        ids: [
            "page01-glyph-entry",
            "page01-glyph-expand",
            "page01-glyph-hook",
        ],
    },
    Page {
        number: 2,
        entry: 0x6736,
        wait: 0x6e5f,
        ids: [
            "page02-glyph-entry",
            "page02-glyph-expand",
            "page02-glyph-hook",
        ],
    },
    Page {
        number: 3,
        entry: 0x6628,
        wait: 0x6d51,
        ids: [
            "page03-glyph-entry",
            "page03-glyph-expand",
            "page03-glyph-hook",
        ],
    },
    Page {
        number: 4,
        entry: 0x6668,
        wait: 0x6d91,
        ids: [
            "page04-glyph-entry",
            "page04-glyph-expand",
            "page04-glyph-hook",
        ],
    },
    Page {
        number: 5,
        entry: 0x663d,
        wait: 0x6d66,
        ids: [
            "page05-glyph-entry",
            "page05-glyph-expand",
            "page05-glyph-hook",
        ],
    },
    Page {
        number: 6,
        entry: 0x6647,
        wait: 0x6d70,
        ids: [
            "page06-glyph-entry",
            "page06-glyph-expand",
            "page06-glyph-hook",
        ],
    },
    Page {
        number: 7,
        entry: 0x5abf,
        wait: 0x62ba,
        ids: [
            "page07-glyph-entry",
            "page07-glyph-expand",
            "page07-glyph-hook",
        ],
    },
];

pub fn sources(
    source: &[u8],
    glyph_count: u16,
    all_main_pages: bool,
) -> Result<Vec<CodeSource>, Box<dyn Error>> {
    let mut result = Vec::new();
    for page in &PAGES[..if all_main_pages { 7 } else { 1 }] {
        result.extend(page_sources(source, glyph_count, page)?);
    }
    Ok(result)
}

fn page_sources(
    source: &[u8],
    glyph_count: u16,
    page: &Page,
) -> Result<Vec<CodeSource>, Box<dyn Error>> {
    let disk = |address: u16| page.number * 0x4000 + usize::from(address) - 0x4000;
    let prelude = hex::decode("5fe61f07577be6e0070707c6185f")?;
    let offset = disk(page.entry);
    let [low, high] = page.wait.to_le_bytes();
    if source.get(offset..offset + 14) != Some(prelude.as_slice())
        || source.get(offset + 43..offset + 46) != Some(&[0xcd, low, high][..])
    {
        return Err("Glyph coordinates or command wait changed".into());
    }
    // The copy tail and wait routine are identical across these pages after call relocation.
    let mut tail = source
        .get(offset + 14..offset + 52)
        .ok_or("Missing glyph tail")?
        .to_vec();
    tail[27..29].fill(0); // Copy-command call target.
    tail[30..32].fill(0); // Command-wait call target.
    let expected_tail = hex::decode(
        "23e526022e01d93a50c8573a51c85fd9c5d5e53a56c8473a55c8cd0000cd0000e1d1c1e13a50",
    )?;
    if tail != expected_tail {
        return Err("Glyph copy tail differs across pages".into());
    }
    let mut wait = Assembler::new();
    wait.emit(LdRImm(A, 2))
        .emit(Call(page.wait + 13))
        .emit(LdAAddr(0xc809))
        .emit(Rrca)
        .emit(JpCond(retro_z80::Condition::C, page.wait))
        .emit(Ret);
    let expected_wait = wait.assemble(page.wait)?.into_bytes();
    if source.get(disk(page.wait)..disk(page.wait) + expected_wait.len())
        != Some(expected_wait.as_slice())
    {
        return Err("VDP command wait implementation changed".into());
    }
    let mut entry = Assembler::new();
    entry.emit(Jp(0x7900));
    let mut hook = Assembler::new();
    hook.emit(Push(StackRegister::Bc))
        .emit(Push(StackRegister::Af))
        .emit(Call(page.wait))
        .emit(Pop(StackRegister::Af))
        .emit(LdRR(E, A))
        .emit(LdRImm(D, 0))
        .emit(Call(0x7800))
        .emit(Pop(StackRegister::Bc))
        .emit(LdRRImm(Register16::De, 0x001b))
        .emit(Jp(page.entry + 14));
    let mut result = vec![CodeSource {
        id: page.ids[0].into(),
        disk_offset: offset,
        program: entry.assemble(page.entry)?,
    }];
    for (id, address, program) in [
        (
            page.ids[1],
            0x7800,
            glyph_expand::assemble(0x7800, glyph_count)?,
        ),
        (page.ids[2], 0x7900, hook.assemble(0x7900)?),
    ] {
        if program.bytes().len() > 0x100
            || source
                .get(disk(address)..disk(address) + program.bytes().len())
                .ok_or("Missing glyph hook placement")?
                .iter()
                .any(|&b| b != 0xe5)
        {
            return Err("Glyph hook exceeds inspected page padding".into());
        }
        result.push(CodeSource {
            id: id.into(),
            disk_offset: disk(address),
            program,
        });
    }
    Ok(result)
}
