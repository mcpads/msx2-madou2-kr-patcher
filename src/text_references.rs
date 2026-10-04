//! Reviewed source references; this is not a whole-game reference census.
use crate::main_source::{parse, word};
use std::error::Error;

pub struct Reference {
    pub id: String,
    pub offset: usize,
    pub end: usize,
}

fn reference(
    source: &[u8],
    id: String,
    offset: usize,
    limit: usize,
) -> Result<Reference, Box<dyn Error>> {
    let (_, end) = parse(source, offset, limit)?;
    Ok(Reference { id, offset, end })
}

fn inline(
    source: &[u8],
    page: usize,
    pointer: usize,
    id: String,
) -> Result<Reference, Box<dyn Error>> {
    let base = (page - 1) * 0x4000;
    let address = word(source, base + pointer)?;
    if !(0x4000..0x8000).contains(&address) {
        return Err("Inline text outside loaded page".into());
    }
    reference(source, id, base + address, base + 0x8000)
}

pub fn extract(source: &[u8]) -> Result<Vec<Reference>, Box<dyn Error>> {
    for (offset, raw) in [
        (0x243c2, "219e561100d4018009edb0"),
        (0x243d0, "21465e110030010010edb0"),
    ] {
        let expected = hex::decode(raw)?;
        if source.get(offset..offset + expected.len()) != Some(expected.as_slice()) {
            return Err("Resident loader changed".into());
        }
    }
    let mut result = Vec::new();
    for table in &TABLES {
        for index in 0..table.count {
            let pointer = word(source, table.offset + index * 2)?;
            let offset = pointer
                .checked_sub(table.base)
                .and_then(|n| n.checked_add(table.offset))
                .ok_or("Resident pointer underflow")?;
            if index >= table.resolved {
                if offset != table.end {
                    return Err("Unresolved resident tail changed".into());
                }
                continue;
            }
            if !(table.offset + table.count * 2..table.end).contains(&offset) {
                return Err("Resident text outside pool".into());
            }
            result.push(reference(
                source,
                format!("{}-{index:03}", table.name),
                offset,
                table.end,
            )?);
        }
    }
    for &(page, common, calls) in DIRECT_CALLS {
        for &address in calls {
            let offset = (page - 1) * 0x4000 + address;
            let code = source.get(offset..offset + 6).ok_or("Call outside disk")?;
            let consumer = word(code, 4)?;
            if code[0] != 0x21
                || code[3] != 0xcd
                || ![common - 0x1e, common - 0xe, common - 7].contains(&consumer)
            {
                return Err("Reviewed direct text call changed".into());
            }
            result.push(inline(
                source,
                page,
                address + 1,
                format!("page-{page:02x}-call-{address:04x}"),
            )?);
        }
    }
    for &(address, count) in EVENT_TABLES {
        for index in 0..count {
            result.push(inline(
                source,
                7,
                address + index * 2,
                format!("page-07-table-{address:04x}-{index}"),
            )?);
        }
    }
    Ok(result)
}

pub struct Table {
    pub name: &'static str,
    pub offset: usize,
    pub count: usize,
    pub resolved: usize,
    pub base: usize,
    pub end: usize,
}

// Source-owned geometry, independently checked against the immutable source hash.
// The banked table's last three pointers lead into unresolved menu records.
pub const TABLES: [Table; 2] = [
    Table {
        name: "common",
        offset: 0x2569e,
        count: 115,
        resolved: 115,
        base: 0xd400,
        end: 0x25e46,
    },
    Table {
        name: "banked",
        offset: 0x25e46,
        count: 204,
        resolved: 201,
        base: 0x3000,
        end: 0x26c32,
    },
];

// Reviewed LD HL,nn / CALL sites.
const DIRECT_CALLS: &[(usize, usize, &[usize])] = &[
    (
        1,
        0x69cd,
        &[
            0x4906, 0x4981, 0x527b, 0x5298, 0x52b1, 0x52d0, 0x52e5, 0x52fa, 0x5315, 0x5330, 0x5354,
            0x5368, 0x537e,
        ],
    ),
    (
        2,
        0x6ad8,
        &[
            0x4906, 0x4981, 0x5281, 0x5287, 0x530e, 0x5315, 0x5321, 0x5342, 0x5349, 0x535e, 0x5385,
            0x539e, 0x53b9, 0x53d7, 0x5407, 0x5437, 0x5460, 0x54b3,
        ],
    ),
    (
        3,
        0x69ca,
        &[
            0x4906, 0x4981, 0x52c5, 0x52e8, 0x52fe, 0x5304, 0x5314, 0x531a, 0x532a, 0x5330, 0x5340,
            0x5346, 0x5350, 0x5360, 0x5384, 0x5393,
        ],
    ),
    (
        4,
        0x6a0a,
        &[
            0x4906, 0x4981, 0x52c0, 0x52d9, 0x52f0, 0x52fe, 0x531b, 0x5321, 0x533f, 0x5348, 0x534e,
            0x537c, 0x538e, 0x539e, 0x53b7, 0x53cc,
        ],
    ),
    (
        5,
        0x69df,
        &[
            0x4906, 0x4981, 0x52de, 0x52f3, 0x531a, 0x5333, 0x535e, 0x5379, 0x5392, 0x53a7, 0x53c0,
            0x53ed, 0x53f4, 0x5412,
        ],
    ),
    (
        6,
        0x69e9,
        &[0x4906, 0x4981, 0x52d6, 0x52fe, 0x533d, 0x535e, 0x5377],
    ),
    (
        7,
        0x5f33,
        &[
            0x4104, 0x4116, 0x41b9, 0x41cb, 0x4411, 0x441f, 0x4428, 0x44a5, 0x49cc, 0x49e3, 0x4a15,
            0x4a6c, 0x4a8b, 0x4aa2, 0x4ac8, 0x4ae3, 0x4af3, 0x4afd, 0x4c24, 0x4ee1,
        ],
    ),
];
const EVENT_TABLES: &[(usize, usize)] = &[
    (0x4289, 2),
    (0x429a, 2),
    (0x42ab, 2),
    (0x42b8, 2),
    (0x42c8, 2),
    (0x42d8, 2),
    (0x4a1f, 3),
];
