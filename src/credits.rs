//! Credits retain contributor glyphs and the original two-stream reveal effect.
use crate::{cutscene_font, media, rle, z80_writes::CodeSource};
use expected_write::{ExpectedWrite, MachineCodeProvenance, WriteIntent};
use fontdue::{Font, FontSettings};
use retro_z80::{Assembler, Instruction, PROFILE_ID, Register16};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
pub type PackedFont = (usize, usize, Vec<u8>);

#[derive(Deserialize)]
struct Draft {
    source_sha256: String,
    input_policy: String,
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    id: String,
    source_role_hex: String,
    source_name_hex: String,
    role: String,
    name: Option<String>,
    review: String,
}
pub struct Installation {
    pub writes: Vec<ExpectedWrite>,
    pub code: Vec<CodeSource>,
    pub fonts: Vec<PackedFont>,
    pub report: serde_json::Value,
}

fn write(
    source: &[u8],
    id: &str,
    offset: usize,
    bytes: Vec<u8>,
    intent: WriteIntent,
) -> Result<ExpectedWrite> {
    Ok(ExpectedWrite {
        id: id.into(),
        owner: "credits".into(),
        purpose: "Source-bounded credits streams and shared ending bitmap storage".into(),
        offset,
        expected_original: source
            .get(offset..offset + bytes.len())
            .ok_or("Credits write outside source")?
            .to_vec(),
        replacement: bytes,
        intent,
    })
}

fn line(text: &str, encoding: &BTreeMap<char, u8>) -> Result<Vec<u8>> {
    let count = text.chars().count();
    if count == 0 || count > 14 || text.chars().any(char::is_control) {
        return Err("Credits line must contain 1..14 visible cells".into());
    }
    let mut result = Vec::new();
    for c in text.chars() {
        result.push(*encoding.get(&c).ok_or("Unmapped credits character")?);
    }
    Ok(result)
}

// Place each replacement row over every old reveal cell of the same row, centered.
// Leading bytes >= 0x80 add (b & 0x3f) pixels to x from 16; bit 6 of the first byte
// selects the title row, so it is kept. Glyph cells are 16 pixels on a 14-cell row.
fn name_rows(source: &[u8], text: &str, encoding: &BTreeMap<char, u8>) -> Result<Vec<u8>> {
    let old_rows = source.split(|&b| b == 0xfe).collect::<Vec<_>>();
    let new_rows = text.split('\n').collect::<Vec<_>>();
    if old_rows.len() != new_rows.len() {
        return Err("Replacement credits name must keep the original row count".into());
    }
    let mut bytes = Vec::new();
    for (index, (old, new)) in old_rows.iter().zip(new_rows).enumerate() {
        let start = old.iter().position(|&b| b < 0x80).ok_or("Empty credits name row")?;
        if old[..start].iter().any(|&b| b >= 0xfd) || old[start..].iter().any(|&b| b >= 0x80) {
            return Err("Credits name row mixes positions and cells".into());
        }
        let old_x = old[..start].iter().map(|&b| usize::from(b & 0x3f)).sum::<usize>();
        let old_cells = old.len() - start;
        let count = new.chars().count();
        if count == 0 || count > 14 || old_x % 16 != 0 {
            return Err("Replacement credits name row must hold 1..14 cells on the cell grid".into());
        }
        let (x, cells) = if count <= old_cells {
            (old_x, old_cells)
        } else {
            let wanted = old_x.saturating_sub((count - old_cells) / 2 * 16);
            (wanted.min(224 - count * 16), count)
        };
        if x > old_x || x + cells * 16 < old_x + old_cells * 16 || x + cells * 16 > 224 {
            return Err("Replacement credits name row cannot cover the old reveal cells".into());
        }
        let left = (cells - count) / 2;
        let padded = format!("{}{}{}", " ".repeat(left), new, " ".repeat(cells - count - left));
        if index > 0 {
            bytes.push(0xfe);
        }
        let title_row = index == 0 && old.first().is_some_and(|&b| b & 0xc0 == 0xc0);
        let mut remaining = x;
        let mut first = true;
        while remaining > 0 || (first && title_row) {
            let step = remaining.min(48);
            bytes.push(0x80 | if first && title_row { 0x40 } else { 0 } | u8::try_from(step)?);
            remaining -= step;
            first = false;
        }
        bytes.extend(line(&padded, encoding)?);
    }
    Ok(bytes)
}

pub fn prepare(
    source: &[u8],
    draft: &[u8],
    ttf: &[u8],
    ending: Vec<u8>,
    correction: Vec<u8>,
) -> Result<Installation> {
    let draft_sha256 = media::sha256(draft);
    let draft: Draft = serde_json::from_slice(draft)?;
    if draft.source_sha256 != media::sha256(source)
        || draft.input_policy != "non-distribution-development"
        || draft.entries.len() != 12
    {
        return Err("Credits source, development policy or card population mismatch".into());
    }
    if media::sha256(ttf) != "d61b60eccb731f8ca9c7da582e4a05a94db66b570471809950aa9a7261b941d6" {
        return Err("Credits font identity mismatch".into());
    }
    if source.get(0x4c20a..0x4c211) != Some(&[0, 0x7e, 0x24, 2, 0x18, 0x7a, 0][..]) {
        return Err("Credits post-load copy changed".into());
    }
    let roles = source[0x4e4f6..0x4e5b9]
        .split(|&b| b == 0xfd)
        .collect::<Vec<_>>();
    let names = source[0x4e5bb..0x4e63a]
        .split(|&b| b == 0xfd)
        .collect::<Vec<_>>();
    if roles.len() != 12
        || names.len() != 12
        || source[0x4e5b9..0x4e5bb] != [0xfd, 0xff]
        || source[0x4e63a] != 0xff
    {
        return Err("Credits stream extents changed".into());
    }
    let mut preserved = BTreeSet::new();
    let mut characters = BTreeSet::new();
    for (i, entry) in draft.entries.iter().enumerate() {
        if entry.id != format!("credit-{:02}", i + 1)
            || hex::decode(&entry.source_role_hex)? != roles[i]
            || hex::decode(&entry.source_name_hex)? != names[i]
            || entry.review != "needs_review"
        {
            return Err("Credits draft/source linkage or development review policy changed".into());
        }
        if entry.name.is_none() {
            preserved.extend(names[i].iter().copied().filter(|&c| c < 0x60));
        }
        characters.extend(entry.role.chars());
        if let Some(name) = &entry.name {
            characters.extend(name.chars().filter(|&c| c != '\n'));
        }
    }
    let mut bitmap = rle::decode(&source[0x5afa7..0x5bbe3], 3586)?;
    let old_extra = rle::decode(&source[0x5bc31..0x5bd0c], 514)?;
    // Apply the native nine-cell overlay before preserving any contributor name.
    for y in 0..16 {
        bitmap[2 + (80 + y) * 32 + 12..2 + (80 + y) * 32 + 30]
            .copy_from_slice(&old_extra[2 + y * 32..2 + y * 32 + 18]);
    }
    let original_effective = bitmap.clone();
    let font = Font::from_bytes(ttf, FontSettings::default())?;
    let mut encoding = BTreeMap::from([(' ', 0x5e)]);
    characters.remove(&' ');
    let available = (0u8..0x60)
        .filter(|c| !preserved.contains(c))
        .collect::<Vec<_>>();
    if characters.len() > available.len() {
        return Err("Credits glyph capacity exceeded".into());
    }
    for (character, code) in characters.into_iter().zip(available) {
        let rows = cutscene_font::cell(&font, character)?;
        for (y, row) in rows.into_iter().enumerate() {
            let offset = 2 + (usize::from(code) / 16 * 16 + y) * 32 + usize::from(code) % 16 * 2;
            bitmap[offset..offset + 2].copy_from_slice(&row.to_be_bytes());
        }
        encoding.insert(character, code);
    }
    for code in preserved.iter().copied().chain(0x60..0x70) {
        for y in 0..16 {
            let p = 2 + (usize::from(code) / 16 * 16 + y) * 32 + usize::from(code) % 16 * 2;
            if bitmap[p..p + 2] != original_effective[p..p + 2] {
                return Err("Credits changed a contributor or reveal glyph".into());
            }
        }
    }
    let mut extra = vec![0; 514];
    extra[..2].copy_from_slice(&512u16.to_le_bytes());
    for y in 0..16 {
        extra[2 + y * 32..2 + y * 32 + 18]
            .copy_from_slice(&bitmap[2 + (80 + y) * 32 + 12..2 + (80 + y) * 32 + 30]);
    }
    let mut role_bytes = Vec::new();
    let mut name_bytes = Vec::new();
    for (i, entry) in draft.entries.iter().enumerate() {
        let split = roles[i].iter().position(|&b| b == 0xfe);
        let has_title = roles[i][..split.unwrap_or(roles[i].len())]
            .iter()
            .any(|&b| b < 0x60 && b != 0x5e);
        if has_title != !entry.role.is_empty() {
            return Err("Credits title population changed".into());
        }
        if has_title {
            role_bytes.extend(line(&entry.role, &encoding)?);
            role_bytes
                .extend_from_slice(&roles[i][split.ok_or("Credits title lost reveal rows")?..]);
        } else {
            role_bytes.extend_from_slice(roles[i]);
        }
        role_bytes.push(0xfd);
        if let Some(name) = &entry.name {
            name_bytes.extend(name_rows(names[i], name, &encoding)?);
        } else {
            name_bytes.extend_from_slice(names[i]);
        }
        name_bytes.push(if i == 11 { 0xff } else { 0xfd });
    }
    role_bytes.push(0xff);
    let mut installation = Installation {
        writes: Vec::new(),
        code: Vec::new(),
        fonts: Vec::new(),
        report: serde_json::Value::Null,
    };
    // Names longer than the source extent move to the unused E5 tail of the page-13 image.
    let names_moved = name_bytes.len() > 0x4e63b - 0x4e5bb;
    let (names_start, names_end) = if names_moved { (0x4fe00, 0x4fff0) } else { (0x4e5bb, 0x4e63b) };
    if names_moved && source[names_start..names_end].iter().any(|&b| b != 0xe5) {
        return Err("Page-13 tail used for relocated credit names is not unused filler".into());
    }
    let mut stream_sizes = Vec::new();
    for (id, start, end, mut bytes) in [
        ("credits-roles", 0x4e4f6, 0x4e5bb, role_bytes),
        ("credits-names", names_start, names_end, name_bytes),
    ] {
        if bytes.len() > end - start {
            return Err(format!("{id} exceeds source stream extent").into());
        }
        stream_sizes.push(bytes.len());
        bytes.extend_from_slice(&source[start + bytes.len()..end]);
        installation
            .writes
            .push(write(source, id, start, bytes, WriteIntent::Data)?);
    }
    let mut combined = Vec::new();
    let mut addresses = Vec::new();
    let mut compressed_sizes = Vec::new();
    for packed in [ending, bitmap, correction, extra] {
        let offset = 0x5a644 + combined.len();
        let compressed = rle::encode(&packed);
        if rle::decode(&compressed, packed.len())? != packed {
            return Err("Credits bitmap round trip failed".into());
        }
        addresses.push(u16::try_from(offset - 0x50000)?);
        compressed_sizes.push(compressed.len());
        combined.extend(compressed);
        installation
            .fonts
            .push((offset, 0x5a644 + combined.len(), packed));
    }
    let used = combined.len();
    if used > 0x5bd0c - 0x5a644 {
        return Err("Ending/credits bitmap container overflow".into());
    }
    combined.extend_from_slice(&source[0x5a644 + used..0x5bd0c]);
    installation.writes.push(write(
        source,
        "ending-credits-bitmaps",
        0x5a644,
        combined,
        WriteIntent::Data,
    )?);
    let mut pointer_patches = vec![
        ("credits-font-address", 0x41db, 0xafa7, addresses[1]),
        ("ending-correction-address", 0x416d, 0xbbe3, addresses[2]),
        ("credits-correction-address", 0x41f1, 0xbc31, addresses[3]),
    ];
    if names_moved {
        pointer_patches.push(("credits-names-address", 0x6376, 0x65bb, 0x7e00));
    }
    for (id, pc, old, new) in pointer_patches {
        let mut original = Assembler::new();
        original.emit(Instruction::LdRRImm(Register16::Hl, old));
        let offset = 0x48000 + usize::from(pc);
        if source[offset..offset + 3] != *original.assemble(pc)?.bytes() {
            return Err("Original bitmap load instruction changed".into());
        }
        let mut assembler = Assembler::new();
        assembler.emit(Instruction::LdRRImm(Register16::Hl, new));
        let program = assembler.assemble(pc)?;
        installation.writes.push(write(
            source,
            id,
            offset,
            program.bytes().to_vec(),
            WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: id.into(),
                isa_profile_id: PROFILE_ID.into(),
            }),
        )?);
        installation.code.push(CodeSource {
            id: id.into(),
            disk_offset: offset,
            program,
        });
    }
    installation.report = serde_json::json!({"draft_sha256":draft_sha256,"encoding":encoding,"cards":12,"stream_bytes":stream_sizes,"bitmap_bytes":compressed_sizes,"container_used":used,"container_capacity":0x5bd0c-0x5a644,"names_relocated":names_moved,"contributor_glyphs_preserved":preserved.len(),"reveal_glyphs_preserved":16,"runtime":"pending","human_review":"pending"});
    Ok(installation)
}

#[cfg(test)]
#[path = "credits_tests.rs"]
mod tests;
