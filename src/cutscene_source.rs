//! Source-owned cutscene references and lossless 16x16 dialogue tokens.
use serde::Serialize;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Token {
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u8>,
    pub raw_hex: String,
}

#[derive(Serialize)]
pub struct Reference {
    pub id: String,
    pub script: usize,
    pub command_address: usize,
    pub pointer_disk_offset: usize,
    pub address: usize,
    pub disk_offset: usize,
    pub initial_xy: [u8; 2],
    pub raw_hex: String,
    pub tokens: Vec<Token>,
}

pub fn text(page: &[u8], address: usize) -> Result<(Vec<Token>, usize)> {
    let start = address.checked_sub(0x4000).ok_or("Dialogue below page")?;
    let mut cursor = start.checked_add(2).ok_or("Dialogue address overflow")?;
    if cursor >= page.len() {
        return Err("Dialogue address outside page".into());
    }
    let mut tokens = Vec::new();
    while cursor < page.len() {
        let begin = cursor;
        let mut byte = page[cursor];
        cursor += 1;
        let mut kind = None;
        let mut offset = None;
        if byte == 0xff {
            byte = *page.get(cursor).ok_or("Truncated dialogue escape")?;
            cursor += 1;
            kind = match byte {
                0xff => Some("end"),
                0xfe => Some("newline"),
                0xfb => {
                    offset = Some(*page.get(cursor).ok_or("Truncated indented newline")?);
                    cursor += 1;
                    Some("newline_offset")
                }
                _ => None,
            };
        }
        let code = if kind.is_none() { Some(byte) } else { None };
        let kind = kind.unwrap_or(match byte {
            0xfd => "delay_tick",
            0xfc => "clear_window",
            _ => "glyph",
        });
        tokens.push(Token {
            kind,
            code,
            offset,
            raw_hex: hex::encode(&page[begin..cursor]),
        });
        if kind == "end" {
            return Ok((tokens, cursor));
        }
    }
    Err("Unterminated dialogue".into())
}

fn scripts(disk: &[u8], page_index: usize, starts: &[usize], name: &str) -> Result<Vec<Reference>> {
    let page_offset = page_index * 0x4000;
    let page = disk
        .get(page_offset..page_offset + 0x4000)
        .ok_or("Missing cutscene page")?;
    let mut references = Vec::new();
    for &start in starts {
        let mut cursor = start.checked_sub(0x4000).ok_or("Script below page")?;
        loop {
            let opcode = *page.get(cursor).ok_or("Missing script terminator")?;
            let size = match opcode {
                0 => break,
                1 => 4,
                2 | 4 => 7,
                3 | 5 | 6 => 3,
                7 => 2,
                _ => return Err("Unknown cutscene opcode".into()),
            };
            let command = page
                .get(cursor..cursor + size)
                .ok_or("Truncated cutscene command")?;
            if opcode == 1 {
                let address = usize::from(u16::from_le_bytes([command[2], command[3]]));
                let (tokens, end) = text(page, address)?;
                let raw = &page[address - 0x4000..end];
                let roundtrip = tokens
                    .iter()
                    .map(|t| t.raw_hex.as_str())
                    .collect::<String>();
                if hex::encode(&raw[2..]) != roundtrip {
                    return Err("Dialogue round trip failed".into());
                }
                references.push(Reference {
                    id: format!("{name}-{start:04x}-{:04x}", cursor + 0x4000),
                    script: start,
                    command_address: cursor + 0x4000,
                    pointer_disk_offset: page_offset + cursor + 2,
                    address,
                    disk_offset: page_offset + address - 0x4000,
                    initial_xy: [raw[0], raw[1]],
                    raw_hex: hex::encode(raw),
                    tokens,
                });
            }
            cursor += size;
        }
    }
    Ok(references)
}

pub fn read(disk: &[u8]) -> Result<Vec<Reference>> {
    for (offset, expected) in [(0x4c0c7, "21d36822d4c9"), (0x4c1a0, "21356922d4c9")] {
        if disk.get(offset..offset + 6) != Some(hex::decode(expected)?.as_slice()) {
            return Err("Interlude/ending entry changed".into());
        }
    }
    let mut references = scripts(disk, 8, &[0x5b50, 0x5c18], "intro")?;
    references.extend(scripts(disk, 0x13, &[0x68d3], "interlude")?);
    references.extend(scripts(disk, 0x13, &[0x6935], "ending")?);
    Ok(references)
}

#[cfg(test)]
#[path = "cutscene_source_tests.rs"]
mod tests;
