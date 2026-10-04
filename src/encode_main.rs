//! Read source-linked draft bytes and emit candidate encoded spans.
use crate::{
    draft_policy,
    main_encoding::Mapping,
    main_source,
    media::sha256,
    menu_draft,
    text_references::{self, Reference},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
};

#[derive(Deserialize)]
struct Draft {
    source_sha256: String,
    policy: String,
    control_policy: String,
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    id: String,
    source: SourceSpan,
    text: String,
    references: Vec<String>,
    review: String,
    context: String,
}

#[derive(Deserialize)]
struct SourceSpan {
    disk_offset: usize,
    raw_hex: String,
    raw_sha256: String,
    annotated_source: String,
}

#[derive(Serialize)]
struct Glyph {
    character: char,
    font_index: usize,
    encoded_hex: String,
}

#[derive(Serialize)]
pub struct EncodedEntry {
    pub id: String,
    pub source_offset: usize,
    pub encoded_hex: String,
}

#[derive(Serialize)]
pub struct EncodingReport {
    kind: &'static str,
    source_sha256: String,
    draft_file_sha256: String,
    glyphs: Vec<Glyph>,
    pub entries: Vec<EncodedEntry>,
    menu_file_sha256: Option<String>,
    reviewed_reference_set_verified: bool,
    whole_game_reference_census_verified: bool,
    source_control_policy_verified: bool,
    menu_wordings_verified: bool,
    menu_wordings_checked: usize,
    game_decoder_installed: bool,
    release_eligible: bool,
}

impl EncodingReport {
    pub fn characters(&self) -> impl Iterator<Item = char> + '_ {
        self.glyphs.iter().map(|glyph| glyph.character)
    }
}

pub fn encode(
    source: &[u8],
    draft_bytes: &[u8],
    menu_bytes: &[u8],
) -> Result<EncodingReport, Box<dyn Error>> {
    let references = text_references::extract(source)?;
    let mut report = encode_references(source, draft_bytes, &references)?;
    let draft: Draft = serde_json::from_slice(draft_bytes)?;
    let wordings = draft
        .entries
        .iter()
        .flat_map(|e| e.references.iter().map(|r| (r.as_str(), e.text.as_str())))
        .collect();
    report.menu_wordings_checked =
        menu_draft::validate(source, menu_bytes, &references, &wordings)?;
    report.menu_wordings_verified = true;
    report.menu_file_sha256 = Some(sha256(menu_bytes));
    Ok(report)
}

fn encode_references(
    source: &[u8],
    draft_bytes: &[u8],
    references: &[Reference],
) -> Result<EncodingReport, Box<dyn Error>> {
    let draft: Draft = serde_json::from_slice(draft_bytes)?;
    if draft.source_sha256 != sha256(source)
        || draft.policy != "non-distribution-development"
        || draft.control_policy != "preserve_flow_and_delays_decode_source_kana"
        || draft.entries.is_empty()
    {
        return Err("Draft source, policy or entries invalid".into());
    }
    let mut groups = BTreeMap::<usize, Vec<&Reference>>::new();
    for reference in references {
        groups.entry(reference.offset).or_default().push(reference);
    }
    if draft.entries.len() != groups.len() {
        return Err("Missing or duplicate source spans".into());
    }
    let mut ids = BTreeSet::new();
    for entry in &draft.entries {
        let raw = hex::decode(&entry.source.raw_hex)?;
        let start = entry.source.disk_offset;
        let end = start.checked_add(raw.len()).ok_or("Source span overflow")?;
        let refs = groups.get(&start).ok_or("Unknown source span")?;
        let (tokens, parsed_end) = main_source::parse(source, start, refs[0].end)?;
        if raw.is_empty()
            || source.get(start..end) != Some(raw.as_slice())
            || sha256(&raw) != entry.source.raw_sha256
            || entry.id != format!("text-{start:05x}")
            || !ids.insert(&entry.id)
            || end != parsed_end
            || entry.references != refs.iter().map(|r| r.id.clone()).collect::<Vec<_>>()
            || entry.source.annotated_source != main_source::annotate(&tokens)
            || entry.review != "needs_review"
            || entry.context.is_empty()
        {
            return Err(format!("Draft source span invalid: {}", entry.id).into());
        }
        draft_policy::validate(&raw, &entry.text)
            .map_err(|error| format!("{}: {error}", entry.id))?;
    }
    let mapping = Mapping::from_texts(draft.entries.iter().map(|e| e.text.as_str()))?;
    if mapping.0.len() + 256 > 896 {
        return Err("Candidate packed font capacity exceeded".into());
    }
    let entries = draft
        .entries
        .iter()
        .map(|entry| {
            Ok(EncodedEntry {
                id: entry.id.clone(),
                source_offset: entry.source.disk_offset,
                encoded_hex: hex::encode(mapping.encode(&entry.text)?),
            })
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    let glyphs = mapping
        .0
        .into_iter()
        .enumerate()
        .map(|(index, (character, bytes))| Glyph {
            character,
            font_index: 256 + index,
            encoded_hex: hex::encode(bytes),
        })
        .collect();
    Ok(EncodingReport {
        kind: "rust-main-encoding-candidate",
        source_sha256: sha256(source),
        draft_file_sha256: sha256(draft_bytes),
        glyphs,
        entries,
        menu_file_sha256: None,
        reviewed_reference_set_verified: true,
        whole_game_reference_census_verified: false,
        source_control_policy_verified: true,
        menu_wordings_verified: false,
        menu_wordings_checked: 0,
        game_decoder_installed: false,
        release_eligible: false,
    })
}

#[cfg(test)]
#[path = "encode_main_tests.rs"]
mod tests;
