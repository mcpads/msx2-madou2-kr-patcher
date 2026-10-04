//! Source-bounded cutscene text/font writes, including the ending post-load copy.
use crate::{
    cutscene_draft::{self, Unit},
    cutscene_font, cutscene_source, rle,
};
use expected_write::{ExpectedWrite, WriteIntent};
use std::{collections::BTreeMap, error::Error};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct Scene {
    name: &'static str,
    pool: std::ops::Range<usize>,
    page: usize,
    font: usize,
    original_size: usize,
    limit: usize,
    packed_size: usize,
}
const SCENES: [Scene; 3] = [
    Scene {
        name: "intro",
        pool: 0x21fe8..0x220fd,
        page: 8,
        font: 0x97465,
        original_size: 2047,
        limit: 0x97e00,
        packed_size: 2562,
    },
    Scene {
        name: "interlude",
        pool: 0x4e63b..0x4e6d0,
        page: 0x13,
        font: 0xa74fa,
        original_size: 1650,
        limit: 0xa7b6c,
        packed_size: 2050,
    },
    Scene {
        name: "ending",
        pool: 0x4e6d0..0x4e803,
        page: 0x13,
        font: 0x5a644,
        original_size: 2403,
        limit: 0x5afa7,
        packed_size: 3074,
    },
];

pub struct Installation {
    pub writes: Vec<ExpectedWrite>,
    pub machine_code: Vec<crate::z80_writes::CodeSource>,
    expected_text: BTreeMap<String, (usize, Vec<u8>)>,
    expected_fonts: Vec<(usize, usize, Vec<u8>)>,
    pub draft_report: serde_json::Value,
}

fn write(
    source: &[u8],
    id: String,
    offset: usize,
    replacement: Vec<u8>,
    intent: WriteIntent,
) -> Result<ExpectedWrite> {
    let original = source
        .get(offset..offset + replacement.len())
        .ok_or("Cutscene write outside disk")?;
    Ok(ExpectedWrite {
        id,
        owner: "cutscenes".into(),
        purpose: "Translated cutscene source-owned extent".into(),
        offset,
        expected_original: original.to_vec(),
        replacement,
        intent,
    })
}

fn font_write(
    source: &[u8],
    name: &str,
    offset: usize,
    original_size: usize,
    limit: usize,
    original_packed: usize,
    packed: &[u8],
) -> Result<ExpectedWrite> {
    let original = source
        .get(offset..offset + original_size)
        .ok_or("Missing source font")?;
    rle::decode(original, original_packed)?;
    if source
        .get(offset + original_size..limit)
        .ok_or("Invalid font padding")?
        .iter()
        .any(|&b| b != 0)
    {
        return Err("Font padding changed".into());
    }
    let compressed = rle::encode(packed);
    if offset + compressed.len() > limit || rle::decode(&compressed, packed.len())? != packed {
        return Err("Font capacity or compression round trip failed".into());
    }
    let mut replacement = compressed.clone();
    if compressed.len() < original_size {
        replacement.extend_from_slice(&original[compressed.len()..]);
    }
    write(
        source,
        format!("{name}-font"),
        offset,
        replacement,
        WriteIntent::Data,
    )
}

pub fn prepare(source: &[u8], draft: &[u8], ttf: &[u8]) -> Result<Installation> {
    let prepared = cutscene_draft::prepare(source, draft)?;
    let fonts = cutscene_font::compile(&prepared, ttf)?;
    let references = cutscene_source::read(source)?;
    let authored: BTreeMap<_, _> = prepared
        .texts
        .iter()
        .map(|t| (t.source_disk_offset, t))
        .collect();
    let mut installation = Installation {
        writes: Vec::new(),
        machine_code: Vec::new(),
        expected_text: BTreeMap::new(),
        expected_fonts: Vec::new(),
        draft_report: prepared.report,
    };
    for scene in &SCENES {
        let refs: Vec<_> = references
            .iter()
            .filter(|r| r.id.starts_with(&format!("{}-", scene.name)))
            .collect();
        let unique: BTreeMap<_, _> = refs.iter().map(|r| (r.disk_offset, *r)).collect();
        let font = fonts.get(scene.name).ok_or("Missing scene font")?;
        let packed = hex::decode(&font.packed_hex)?;
        installation.writes.push(font_write(
            source,
            scene.name,
            scene.font,
            scene.original_size,
            scene.limit,
            scene.packed_size,
            &packed,
        )?);
        installation
            .expected_fonts
            .push((scene.font, scene.limit, packed));
        let mut cursor = scene.pool.start;
        let mut data = Vec::new();
        for (&offset, original) in &unique {
            if cursor != offset {
                return Err("Unknown gap or overlap in source text pool".into());
            }
            cursor += hex::decode(&original.raw_hex)?.len();
            let text = authored
                .get(&offset)
                .ok_or("Missing prepared cutscene text")?;
            let address = scene.pool.start + data.len() - (scene.page - 1) * 0x4000;
            if !(0x4000..0x8000).contains(&address) {
                return Err("Relocated cutscene text left page".into());
            }
            let mut raw = text.initial_xy.to_vec();
            for unit in &text.units {
                match unit {
                    Unit::Glyph(c) => {
                        raw.push(*font.encoding.get(c).ok_or("Unmapped cutscene glyph")?)
                    }
                    Unit::Control(bytes) => raw.extend_from_slice(bytes),
                }
            }
            data.extend_from_slice(&raw);
            for reference in refs.iter().filter(|r| r.disk_offset == offset) {
                installation
                    .expected_text
                    .insert(reference.id.clone(), (address, raw.clone()));
                installation.writes.push(write(
                    source,
                    format!("{}-pointer", reference.id),
                    reference.pointer_disk_offset,
                    (address as u16).to_le_bytes().to_vec(),
                    WriteIntent::Metadata,
                )?);
            }
        }
        if cursor != scene.pool.end || data.len() > scene.pool.len() {
            return Err("Cutscene pool extent changed or overflowed".into());
        }
        data.extend_from_slice(&source[scene.pool.start + data.len()..scene.pool.end]);
        installation.writes.push(write(
            source,
            format!("{}-text-pool", scene.name),
            scene.pool.start,
            data,
            WriteIntent::Data,
        )?);
    }
    if source.get(0x4c186..0x4c18d) != Some(&[0, 0x7c, 4, 2, 0x14, 0x78, 0][..]) {
        return Err("Ending post-load copy changed".into());
    }
    let ending = hex::decode(&fonts["ending"].packed_hex)?;
    let correction = ending_correction(&ending)?;
    installation.writes.push(font_write(
        source,
        "ending-correction",
        0x5bbe3,
        78,
        0x5bc31,
        514,
        &correction,
    )?);
    installation
        .expected_fonts
        .push((0x5bbe3, 0x5bc31, correction));
    Ok(installation)
}

fn ending_correction(ending: &[u8]) -> Result<Vec<u8>> {
    let mut correction = vec![0; 514];
    correction[..2].copy_from_slice(&512u16.to_le_bytes());
    for row in 0..16 {
        let offset = 2 + (64 + row) * 32 + 10;
        correction[2 + row * 32..4 + row * 32].copy_from_slice(
            ending
                .get(offset..offset + 2)
                .ok_or("Ending font lacks C5 cell")?,
        );
    }
    Ok(correction)
}

impl Installation {
    pub fn add_credits(&mut self, source: &[u8], draft: &[u8], ttf: &[u8]) -> Result<()> {
        let ending = self
            .expected_fonts
            .iter()
            .find(|f| f.0 == 0x5a644)
            .ok_or("Missing ending font")?
            .2
            .clone();
        let correction = self
            .expected_fonts
            .iter()
            .find(|f| f.0 == 0x5bbe3)
            .ok_or("Missing ending correction")?
            .2
            .clone();
        let credits = crate::credits::prepare(source, draft, ttf, ending, correction)?;
        self.writes
            .retain(|w| w.id != "ending-font" && w.id != "ending-correction-font");
        self.expected_fonts
            .retain(|f| f.0 != 0x5a644 && f.0 != 0x5bbe3);
        self.writes.extend(credits.writes);
        self.expected_fonts.extend(credits.fonts);
        self.machine_code.extend(credits.code);
        self.draft_report["credits"] = credits.report;
        Ok(())
    }
    pub fn verify(&self, candidate: &[u8]) -> Result<()> {
        let refs = cutscene_source::read(candidate)?;
        if refs.len() != self.expected_text.len() {
            return Err("Final cutscene reference count changed".into());
        }
        for reference in refs {
            let (address, raw) = self
                .expected_text
                .get(&reference.id)
                .ok_or("Unknown final cutscene reference")?;
            if reference.address != *address || hex::decode(&reference.raw_hex)? != *raw {
                return Err("Final cutscene text differs from prepared input".into());
            }
        }
        for (offset, limit, packed) in &self.expected_fonts {
            let input = candidate.get(*offset..*limit).ok_or("Missing final font")?;
            if rle::decode_prefix(input, packed.len())?.0 != *packed {
                return Err("Final font differs from prepared pixels".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "cutscene_install_tests.rs"]
mod tests;
