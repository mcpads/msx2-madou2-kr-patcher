//! Validate cutscene drafts against source references before font allocation or writes.
use crate::{
    cutscene_source::{self, Reference},
    media,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Deserialize)]
struct Draft {
    source_sha256: String,
    input_policy: String,
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    id: String,
    group: String,
    source_disk_offset: usize,
    source_raw_sha256: String,
    references: Vec<String>,
    text: String,
    state: String,
    #[serde(default)]
    layout: Layout,
}
#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Layout {
    #[serde(skip_serializing_if = "Option::is_none")]
    initial_xy: Option<[u8; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    newline_offsets: Option<Vec<u8>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Unit {
    Glyph(char),
    Control(Vec<u8>),
}
#[derive(Serialize)]
pub struct PreparedText {
    pub source_disk_offset: usize,
    pub group: String,
    pub initial_xy: [u8; 2],
    pub units: Vec<Unit>,
}
#[derive(Serialize)]
pub struct Prepared {
    pub report: Value,
    pub texts: Vec<PreparedText>,
}

fn prepare_entry(
    entry: &Entry,
    original: &Reference,
) -> Result<(PreparedText, Value, BTreeSet<char>)> {
    if entry.state != "needs_review" || !entry.text.ends_with("<END>") {
        return Err("Invalid development review state or terminator".into());
    }
    let initial_xy = entry.layout.initial_xy.unwrap_or(original.initial_xy);
    let source_offsets: Vec<_> = original.tokens.iter().filter_map(|t| t.offset).collect();
    let offsets = entry
        .layout
        .newline_offsets
        .as_ref()
        .unwrap_or(&source_offsets);
    if offsets.len() != source_offsets.len() {
        return Err("Newline layout count mismatch".into());
    }
    let mut controls = original.tokens.iter().filter(|t| t.kind != "glyph");
    let mut offset_index = 0;
    let (mut x, mut y) = (usize::from(initial_xy[0]), usize::from(initial_xy[1]));
    let mut right_edge = x;
    let mut glyphs = BTreeSet::new();
    let mut count = 0;
    let mut units = Vec::new();
    let mut chars = entry.text.chars();
    while let Some(c) = chars.next() {
        if c == '<' {
            let mut label = String::new();
            let mut closed = false;
            for c in chars.by_ref() {
                if c == '>' {
                    closed = true;
                    break;
                }
                label.push(c);
            }
            if !closed {
                return Err("Unterminated draft control".into());
            }
            let control = controls.next().ok_or("Extra draft control")?;
            let expected = match control.offset {
                Some(n) => format!("{}:{n}", control.kind.to_uppercase()),
                None => control.kind.to_uppercase(),
            };
            if label != expected {
                return Err("Protected cutscene controls changed".into());
            }
            let mut raw = hex::decode(&control.raw_hex)?;
            match control.kind {
                "clear_window" => x = 16,
                "newline" | "newline_offset" => {
                    x = 16;
                    y += 24;
                    if control.kind == "newline_offset" {
                        let offset = offsets[offset_index];
                        offset_index += 1;
                        *raw.last_mut().ok_or("Empty newline control")? = offset;
                        x += usize::from(offset);
                    }
                }
                _ => (),
            }
            units.push(Unit::Control(raw));
        } else {
            if matches!(c, '>' | '\r' | '\n' | '\t') {
                return Err("Implicit draft layout".into());
            }
            if !(16 <= x && x + 16 <= 240 && 120 <= y && y + 16 <= 184) {
                return Err(
                    format!("Cutscene layout overflow: {} {c:?} at {x},{y}", entry.id).into(),
                );
            }
            glyphs.insert(c);
            count += 1;
            right_edge = right_edge.max(x + 16);
            x += 16;
            units.push(Unit::Glyph(c));
        }
    }
    if controls.next().is_some() {
        return Err("Missing draft control".into());
    }
    let measurement = json!({"id":entry.id,"glyphs":count,"right_edge":right_edge,
        "last_y":y,"explicit_layout":entry.layout});
    Ok((
        PreparedText {
            source_disk_offset: entry.source_disk_offset,
            group: entry.group.clone(),
            initial_xy,
            units,
        },
        measurement,
        glyphs,
    ))
}

pub fn prepare(source: &[u8], draft_bytes: &[u8]) -> Result<Prepared> {
    let draft: Draft = serde_json::from_slice(draft_bytes)?;
    let references = cutscene_source::read(source)?;
    validate(source, draft_bytes, draft, &references)
}

fn validate(
    source: &[u8],
    draft_bytes: &[u8],
    draft: Draft,
    references: &[Reference],
) -> Result<Prepared> {
    if draft.source_sha256 != media::sha256(source)
        || draft.input_policy != "non-distribution-development"
    {
        return Err("Cutscene draft source or input policy mismatch".into());
    }
    let mut baseline: BTreeMap<usize, Vec<&Reference>> = BTreeMap::new();
    for reference in references {
        baseline
            .entry(reference.disk_offset)
            .or_default()
            .push(reference);
    }
    if draft.entries.len() != baseline.len() {
        return Err("Missing or extra cutscene spans".into());
    }
    let mut seen = BTreeSet::new();
    let mut glyphs: BTreeMap<String, BTreeSet<char>> = BTreeMap::new();
    let mut texts = Vec::new();
    let mut measurements = Vec::new();
    for entry in &draft.entries {
        if !seen.insert(entry.source_disk_offset) {
            return Err("Duplicate cutscene span".into());
        }
        let aliases = baseline
            .get(&entry.source_disk_offset)
            .ok_or("Unknown cutscene span")?;
        let original = aliases[0];
        let group = original.id.split('-').next().ok_or("Missing scene name")?;
        if entry.id != format!("cutscene-{:05x}", entry.source_disk_offset)
            || entry.group != group
            || entry.source_raw_sha256 != media::sha256(&hex::decode(&original.raw_hex)?)
            || entry.references != aliases.iter().map(|r| r.id.clone()).collect::<Vec<_>>()
        {
            return Err("Cutscene source span or aliases changed".into());
        }
        let (text, measurement, chars) = prepare_entry(entry, original)?;
        texts.push(text);
        measurements.push(measurement);
        glyphs.entry(group.to_string()).or_default().extend(chars);
    }
    let mut fonts = BTreeMap::new();
    for (group, chars) in glyphs {
        let (original_cells, capacity) = match group.as_str() {
            "intro" => (80, 96),
            "interlude" => (64, 64),
            "ending" => (96, 96),
            _ => return Err("Unknown cutscene font".into()),
        };
        if chars.len() > capacity {
            return Err("Cutscene glyph capacity exceeded".into());
        }
        fonts.insert(group, json!({"count":chars.len(),"characters":chars.iter().collect::<String>(),"original_cells":original_cells}));
    }
    Ok(Prepared {
        texts,
        report: json!({"source_sha256":media::sha256(source),
        "draft_file_sha256":media::sha256(draft_bytes),"source_references":references.len(),
        "source_spans":baseline.len(),"source_and_controls_match":true,"glyphs_by_font":fonts,
        "overflow":[],"measurements":measurements,"runtime":"not_tested",
        "human_review":"pending","release_eligible":false}),
    })
}

#[cfg(test)]
#[path = "cutscene_draft_tests.rs"]
mod tests;
