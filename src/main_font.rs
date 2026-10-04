//! Compile the candidate packed font from source glyphs and validated text order.
use crate::{encode_main::EncodingReport, media::sha256, rle};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
};

pub fn compile(
    source: &[u8],
    encoding: &EncodingReport,
    bdf: &[u8],
) -> Result<Vec<u8>, Box<dyn Error>> {
    if sha256(bdf) != "2a6fd090ac6d24f7392d6cc49db02ce54b9d1c01048bf8c97cbcdbc5a885cb15" {
        return Err("Candidate BDF identity mismatch".into());
    }
    let characters: Vec<_> = encoding.characters().collect();
    let glyphs = read_glyphs(std::str::from_utf8(bdf)?, &characters)?;
    let original = boot_font(source)?;
    let mut packed = vec![0; 8192];
    // Preserve the original scratch/wait-arrow cells, outside packed glyph storage.
    let expanded: Vec<u8> = original
        .iter()
        .flat_map(|byte| {
            (0..4).map(move |pair| {
                let shift = 6 - pair * 2;
                (if byte & (2 << shift) != 0 { 0xf0 } else { 0 })
                    | (if byte & (1 << shift) != 0 { 0x0f } else { 0 })
            })
        })
        .collect();
    packed[0xc00..0x1000].copy_from_slice(&expanded[0xc00..0x1000]);
    for index in 0..256 + characters.len() {
        let address = index * 8 + if index >= 384 { 0x400 } else { 0 };
        let target = packed
            .get_mut(address..address + 8)
            .ok_or("Packed font capacity exceeded")?;
        if index < 256 {
            for (row, byte) in target.iter_mut().enumerate() {
                *byte = original[(index / 32 * 8 + row) * 32 + index % 32];
            }
        } else {
            target.copy_from_slice(&glyphs[&characters[index - 256]]);
        }
    }
    Ok(packed)
}

fn boot_font(source: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    for (offset, expected) in [
        (0x20037, "3e0acd034d2109b31100803e01cdd348"),
        (0x20047, "2100801100e03e00cd1449"),
        (0x20052, "cd4d4c00784008001800cd1c50"),
    ] {
        let bytes = hex::decode(expected)?;
        if source.get(offset..offset + bytes.len()) != Some(bytes.as_slice()) {
            return Err("Boot font consumer changed".into());
        }
    }
    let page = usize::from(source[0x20038]);
    let address = usize::from(u16::from_le_bytes([source[0x2003d], source[0x2003e]]));
    let offset = page * 0x4000 + address - 0x8000;
    let input = source
        .get(offset..(page + 1) * 0x4000)
        .ok_or("Font source outside disk")?;
    let (decoded, _) = rle::decode_prefix(input, 2050)?;
    if decoded[..2] != [0, 8] {
        return Err("Original font bitmap size changed".into());
    }
    Ok(decoded[2..].to_vec())
}

pub(crate) fn read_glyphs(
    bdf: &str,
    characters: &[char],
) -> Result<BTreeMap<char, [u8; 8]>, Box<dyn Error>> {
    let wanted: BTreeSet<_> = characters.iter().copied().collect();
    let mut glyphs = BTreeMap::new();
    for block in bdf.split("STARTCHAR ").skip(1) {
        let lines: Vec<_> = block.lines().collect();
        let field = |prefix: &str| {
            lines
                .iter()
                .find_map(|line| line.strip_prefix(prefix))
                .ok_or("Missing BDF field")
        };
        let code: u32 = field("ENCODING ")?.parse()?;
        let Some(character) = char::from_u32(code).filter(|c| wanted.contains(c)) else {
            continue;
        };
        let bounds = field("BBX ")?
            .split_whitespace()
            .map(str::parse::<i32>)
            .collect::<Result<Vec<_>, _>>()?;
        let [width, height, x, y] = bounds.as_slice() else {
            return Err("Invalid BDF bounds".into());
        };
        let top = 7 - y - height;
        if *width < 0 || *height < 0 || *x < 0 || top < 0 || x + width > 8 || top + height > 8 {
            return Err(format!("Glyph exceeds cell: {character}").into());
        }
        let start = lines
            .iter()
            .position(|line| *line == "BITMAP")
            .ok_or("Missing BDF bitmap")?
            + 1;
        let end = lines
            .iter()
            .position(|line| *line == "ENDCHAR")
            .ok_or("Missing BDF end")?;
        let rows = lines.get(start..end).ok_or("Invalid BDF bitmap range")?;
        if rows.len() != *height as usize {
            return Err("BDF row count mismatch".into());
        }
        let mut cell = [0; 8];
        for (row, encoded) in rows.iter().enumerate() {
            let bits = hex::decode(encoded)?;
            if bits.len() != (*width as usize).div_ceil(8) {
                return Err("BDF row width mismatch".into());
            }
            for column in 0..*width as usize {
                if bits[column / 8] & (0x80 >> (column % 8)) != 0 {
                    cell[top as usize + row] |= 0x80 >> (*x as usize + column);
                }
            }
        }
        if character != ' ' && cell == [0; 8] {
            return Err("Unexpected blank glyph".into());
        }
        if glyphs.insert(character, cell).is_some() {
            return Err("Duplicate BDF glyph".into());
        }
    }
    if glyphs.len() != wanted.len() {
        return Err("Missing requested BDF glyphs".into());
    }
    Ok(glyphs)
}

#[cfg(test)]
#[path = "main_font_tests.rs"]
mod tests;
