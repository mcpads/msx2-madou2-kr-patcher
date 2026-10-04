//! Reproduce the development cutscene glyphs and their source-compatible cell layout.
use crate::{
    cutscene_draft::{Prepared, Unit},
    media, rle,
};
use fontdue::{Font, FontSettings};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
};

#[derive(Serialize)]
pub struct SceneFont {
    pub encoding: BTreeMap<char, u8>,
    pub packed_hex: String,
    pub packed_sha256: String,
    pub compressed_hex: String,
    pub compressed_bytes: usize,
}

pub(crate) fn cell(font: &Font, character: char) -> Result<[u16; 16], Box<dyn Error>> {
    if font.lookup_glyph_index(character) == 0 {
        return Err(format!("Cutscene font has no character {character:?}").into());
    }
    let (metrics, bitmap) = font.rasterize(character, 16.0);
    // The pinned font's 16px ascent is 12, matching Pillow's default ascender anchor.
    let top = 12 - metrics.ymin - metrics.height as i32;
    if metrics.xmin < 0
        || top < 0
        || metrics.xmin + metrics.width as i32 > 16
        || top + metrics.height as i32 > 16
    {
        return Err(format!("Cutscene glyph exceeds cell: {character:?}").into());
    }
    let mut rows = [0u16; 16];
    for y in 0..metrics.height {
        for x in 0..metrics.width {
            if bitmap[y * metrics.width + x] >= 128 {
                rows[top as usize + y] |= 1 << (15 - metrics.xmin as usize - x);
            }
        }
    }
    if character != ' ' && rows.iter().all(|&row| row == 0) {
        return Err(format!("Unexpected empty cutscene glyph: {character:?}").into());
    }
    Ok(rows)
}

fn pack(cells: usize, glyphs: &BTreeMap<u8, [u16; 16]>) -> Result<Vec<u8>, Box<dyn Error>> {
    let size = cells
        .checked_mul(32)
        .filter(|&s| s <= u16::MAX as usize)
        .ok_or("Cutscene bitmap size overflow")?;
    let mut packed = vec![0; 2 + size];
    packed[..2].copy_from_slice(&(size as u16).to_le_bytes());
    for (&code, rows) in glyphs {
        let index = usize::from(
            code.checked_sub(0x80)
                .ok_or("Invalid cutscene glyph code")?,
        );
        if index >= cells || matches!(code, 0xfc | 0xfd | 0xff) {
            return Err("Cutscene glyph code exceeds cells or collides with control".into());
        }
        for (y, row) in rows.iter().enumerate() {
            let offset = 2 + (index / 16 * 16 + y) * 32 + index % 16 * 2;
            packed[offset..offset + 2].copy_from_slice(&row.to_be_bytes());
        }
    }
    Ok(packed)
}

pub fn compile(
    prepared: &Prepared,
    font_bytes: &[u8],
) -> Result<BTreeMap<String, SceneFont>, Box<dyn Error>> {
    if media::sha256(font_bytes)
        != "d61b60eccb731f8ca9c7da582e4a05a94db66b570471809950aa9a7261b941d6"
    {
        return Err("Cutscene TTF identity mismatch".into());
    }
    let font = Font::from_bytes(font_bytes, FontSettings::default())?;
    let mut groups: BTreeMap<&str, BTreeSet<char>> = BTreeMap::new();
    for text in &prepared.texts {
        let chars = groups.entry(&text.group).or_default();
        for unit in &text.units {
            if let Unit::Glyph(c) = unit {
                chars.insert(*c);
            }
        }
    }
    let mut fonts = BTreeMap::new();
    for (group, characters) in groups {
        let cells = match group {
            "intro" | "ending" => 96,
            "interlude" => 64,
            _ => return Err("Unknown cutscene font group".into()),
        };
        let pinned = if group == "ending" {
            Some(('안', 0xc5))
        } else {
            None
        };
        if characters.len() > cells || pinned.is_some_and(|(c, _)| !characters.contains(&c)) {
            return Err("Cutscene font capacity or ending correction character mismatch".into());
        }
        let mut codes = (0x80..0x80 + cells as u16)
            .map(|c| c as u8)
            .filter(|c| !pinned.is_some_and(|(_, reserved)| *c == reserved));
        let mut encoding = BTreeMap::new();
        let mut glyphs = BTreeMap::new();
        for c in characters {
            let code = if let Some((_, code)) = pinned.filter(|(fixed, _)| *fixed == c) {
                code
            } else {
                codes.next().ok_or("Cutscene font capacity exceeded")?
            };
            encoding.insert(c, code);
            glyphs.insert(code, cell(&font, c)?);
        }
        let packed = pack(cells, &glyphs)?;
        let compressed = rle::encode(&packed);
        fonts.insert(
            group.to_owned(),
            SceneFont {
                encoding,
                packed_sha256: media::sha256(&packed),
                packed_hex: hex::encode(packed),
                compressed_bytes: compressed.len(),
                compressed_hex: hex::encode(compressed),
            },
        );
    }
    Ok(fonts)
}

#[cfg(test)]
#[path = "cutscene_font_tests.rs"]
mod tests;
