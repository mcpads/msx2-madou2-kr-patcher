//! Replace only the four title cells; retain the numeral and native animation.
use crate::{cutscene_install::Installation, media::sha256, rle};
use expected_write::{ExpectedWrite, WriteIntent};
use serde::Deserialize;
use std::error::Error;

#[derive(Deserialize)]
struct Draft {
    source_sha256: String,
    graphics_sha256: String,
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    id: String,
    source: String,
    text: String,
    status: String,
    graphic_text: String,
    pronunciation_badges: Vec<String>,
}

#[derive(Deserialize)]
struct Graphics {
    width: usize,
    height: usize,
    graphic_text: String,
    pronunciation_badges: Vec<String>,
    rows: Vec<String>,
}

pub fn install(
    source: &[u8],
    draft: &[u8],
    graphics: &[u8],
    result: &mut Installation,
) -> Result<(), Box<dyn Error>> {
    let draft: Draft = serde_json::from_slice(draft)?;
    if draft.source_sha256 != sha256(source)
        || draft.entries.len() != 1
        || sha256(graphics) != draft.graphics_sha256
    {
        return Err("Title source or graphics identity changed".into());
    }
    let entry = &draft.entries[0];

    if entry.id != "opening-title"
        || entry.source != "魔導物語2"
        || entry.status != "needs_review"
        || entry.text != "마도전기2"
        || entry.graphic_text != "魔導傳記2"
        || entry.pronunciation_badges != ["마", "도", "전", "기"]
    {
        return Err(
            "Title must identify the four Han characters and Korean pronunciation badges".into(),
        );
    }
    let offset = 0x3f446;
    let original = source
        .get(offset..offset + 2028)
        .ok_or("Missing title bitmap")?;
    let raw = rle::decode(original, 5120)?;
    if sha256(&raw) != "1a429a6d3f192cb7de0418abc4401a5a7d8aa44980d7277ec3e1e602bfc006e2" {
        return Err("Title pixel identity changed".into());
    }
    let graphics: Graphics = serde_json::from_slice(graphics)?;
    if graphics.width != 160
        || graphics.height != 40
        || graphics.graphic_text != "魔導傳記"
        || graphics.pronunciation_badges != entry.pronunciation_badges
        || graphics.rows.len() != 40
        || graphics
            .rows
            .iter()
            .any(|r| r.len() != 160 || r.bytes().any(|b| b != b'0' && b != b'9'))
    {
        return Err("Invalid title graphics dimensions, palette or labels".into());
    }
    let mut pixels = raw.clone();
    for (y, row) in graphics.rows.iter().enumerate() {
        for (x, pair) in row.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            pixels[y * 128 + x] = ((pair[0] - b'0') << 4) | (pair[1] - b'0');
        }
    }
    for column in 0..4 {
        if !graphics
            .rows
            .iter()
            .any(|r| r.as_bytes()[column * 40..column * 40 + 40].contains(&b'9'))
        {
            return Err("Empty title graphic cell".into());
        }
    }
    if pixels
        .as_chunks::<128>()
        .0
        .iter()
        .zip(raw.as_chunks::<128>().0.iter())
        .any(|(a, b)| a[80..] != b[80..])
    {
        return Err("Title numeral or protected pixels changed".into());
    }
    let mut replacement = rle::encode(&pixels);
    if replacement.len() > original.len() || rle::decode(&replacement, 5120)? != pixels {
        return Err("Title compressed capacity or pixel round trip failed".into());
    }
    // Preserve unused bytes after the new terminator; no adjacent asset is touched.
    replacement.extend_from_slice(&original[replacement.len()..]);
    result.writes.push(ExpectedWrite {
        id: "opening-title-bitmap".into(),
        owner: "title".into(),
        purpose: "Korean title cells with original numeral and animation".into(),
        offset,
        expected_original: original.to_vec(),
        replacement,
        intent: WriteIntent::Data,
    });
    Ok(())
}
