//! Validate menu wording against source CE00 records and the main draft.
use crate::{main_encoding::Token, main_source, media::sha256, text_references::Reference};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
};
use unicode_normalization::UnicodeNormalization;

#[derive(Deserialize)]
struct Draft {
    source_sha256: String,
    policy: String,
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    id: String,
    source_raw_sha256: String,
    text: String,
    review: String,
}

pub fn source_names(source: &[u8]) -> Result<BTreeSet<String>, Box<dyn Error>> {
    const START: usize = 0x26c32;
    const END: usize = 0x26c90;
    if source.get(0x243ac..0x243b7) != Some(hex::decode("21326c1100ce018000edb0")?.as_slice()) {
        return Err("Menu loader changed".into());
    }
    let mut names: BTreeSet<usize> = (47..70).collect();
    for index in 0..20 {
        let offset = main_source::word(source, START + index * 2)?
            .checked_sub(0xce00)
            .and_then(|n| n.checked_add(START))
            .ok_or("Menu pointer underflow")?;
        if index >= 17 {
            if offset != END {
                return Err("Unresolved menu tail changed".into());
            }
            continue;
        }
        if !(START + 40..END).contains(&offset) {
            return Err("Menu record outside pool".into());
        }
        let (count, labels) = if index < 8 {
            let count = *source.get(offset).ok_or("Missing menu count")? as usize;
            if !(1..=15).contains(&count) {
                return Err("Invalid menu count".into());
            }
            (count, offset + 1)
        } else {
            (2, offset)
        };
        if labels + count > END {
            return Err("Menu record exceeds pool".into());
        }
        for label in source
            .get(labels..labels + count)
            .ok_or("Missing menu labels")?
        {
            if *label >= 201 {
                return Err("Menu references unresolved banked name".into());
            }
            names.insert(*label as usize);
        }
    }
    Ok(names
        .into_iter()
        .map(|n| format!("banked-{n:03}"))
        .collect())
}

pub fn validate(
    source: &[u8],
    menu_bytes: &[u8],
    references: &[Reference],
    main_wordings: &BTreeMap<&str, &str>,
) -> Result<usize, Box<dyn Error>> {
    let names = source_names(source)?;
    let draft: Draft = serde_json::from_slice(menu_bytes)?;
    let originals = references
        .iter()
        .map(|r| (r.id.as_str(), r))
        .collect::<BTreeMap<_, _>>();
    validate_names(source, draft, &names, &originals, main_wordings)
}

fn validate_names(
    source: &[u8],
    draft: Draft,
    names: &BTreeSet<String>,
    originals: &BTreeMap<&str, &Reference>,
    main_wordings: &BTreeMap<&str, &str>,
) -> Result<usize, Box<dyn Error>> {
    if draft.source_sha256 != sha256(source)
        || draft.policy != "non-distribution-development"
        || draft.entries.len() != names.len()
    {
        return Err("Menu source, policy or coverage mismatch".into());
    }
    let mut seen = BTreeSet::new();
    for entry in draft.entries {
        if !names.contains(&entry.id) || !seen.insert(entry.id.clone()) {
            return Err("Unknown or duplicate menu name".into());
        }
        let reference = originals
            .get(entry.id.as_str())
            .ok_or("Menu source reference missing")?;
        let raw = source
            .get(reference.offset..reference.end)
            .ok_or("Menu source out of bounds")?;
        let (tokens, end) = main_source::parse(raw, 0, raw.len())?;
        if end != raw.len()
            || tokens
                .iter()
                .any(|t| !matches!(t, Token::Glyph(_) | Token::Command(b'@', None) | Token::End))
            || raw.windows(2).any(|w| w == b"$$")
        {
            return Err("Source name violates menu grammar".into());
        }
        if entry.source_raw_sha256 != sha256(raw) || entry.review != "needs_review" {
            return Err("Menu source or review changed".into());
        }
        if entry.text.is_empty()
            || !entry.text.nfc().eq(entry.text.chars())
            || entry
                .text
                .chars()
                .any(|c| c.is_control() || "<>$".contains(c))
            || entry.text.chars().count() > 10
        {
            return Err("Menu wording violates normalized ten-cell layout".into());
        }
        let expected = format!("{}<END>", entry.text);
        if main_wordings.get(entry.id.as_str()).copied() != Some(expected.as_str()) {
            return Err(format!("Main/menu wording conflict: {}", entry.id).into());
        }
    }
    Ok(seen.len())
}

#[cfg(test)]
#[path = "menu_draft_tests.rs"]
mod tests;
