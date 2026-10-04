//! Generate candidate resident writes directly from validated translation inputs.
use crate::{
    encode_main::{self, EncodingReport},
    main_source::word,
    resident_plan::{self, AnalysisPlan, AnalysisWrite},
    text_references::{self, Reference, TABLES, Table},
};
use serde::Serialize;
use std::{collections::BTreeMap, error::Error};

#[derive(Serialize)]
struct PoolBudget {
    used: usize,
    capacity: usize,
    remaining: usize,
    unique_spans: usize,
}

#[derive(Serialize)]
pub struct PreparedResident {
    #[serde(flatten)]
    pub plan: AnalysisPlan,
    encoding: EncodingReport,
    pools: BTreeMap<String, PoolBudget>,
    verified_references: usize,
    decoder: &'static str,
    font_loader: &'static str,
    page_local_text: &'static str,
    release_eligible: bool,
}

fn pool_writes(
    source: &[u8],
    table: &Table,
    refs: &[&Reference],
    encoded: &BTreeMap<usize, Vec<u8>>,
) -> Result<(Vec<AnalysisWrite>, PoolBudget), Box<dyn Error>> {
    let start = table.offset + table.count * 2;
    let capacity = table
        .end
        .checked_sub(start)
        .ok_or("Invalid resident pool")?;
    let mut pool = vec![0; capacity];
    let mut locations = BTreeMap::new();
    let mut sorted = refs.to_vec();
    sorted.sort_by_key(|r| r.offset);
    let mut used: usize = 0;
    for reference in sorted {
        if locations.contains_key(&reference.offset) {
            continue;
        }
        let bytes = encoded
            .get(&reference.offset)
            .ok_or("Missing encoded resident span")?;
        let end = used.checked_add(bytes.len()).ok_or("Pool overflow")?;
        if bytes.is_empty() || end > capacity {
            return Err(format!("Resident pool capacity exceeded: {}", table.name).into());
        }
        pool[used..end].copy_from_slice(bytes);
        locations.insert(reference.offset, start + used);
        used = end;
    }
    let mut writes = vec![AnalysisWrite {
        name: format!("{}-text", table.name),
        offset: start,
        expected_hex: hex::encode(source.get(start..table.end).ok_or("Pool outside source")?),
        replacement_hex: hex::encode(pool),
    }];
    if refs.len() != table.resolved {
        return Err("Resident reference count mismatch".into());
    }
    for (index, reference) in refs.iter().enumerate() {
        if reference.id != format!("{}-{index:03}", table.name) {
            return Err("Resident reference ordering changed".into());
        }
        let address = u16::try_from(table.base + locations[&reference.offset] - table.offset)?;
        let offset = table.offset + index * 2;
        writes.push(AnalysisWrite {
            name: reference.id.clone(),
            offset,
            expected_hex: hex::encode(
                source
                    .get(offset..offset + 2)
                    .ok_or("Pointer outside source")?,
            ),
            replacement_hex: hex::encode(address.to_le_bytes()),
        });
    }
    Ok((
        writes,
        PoolBudget {
            used,
            capacity,
            remaining: capacity - used,
            unique_spans: locations.len(),
        },
    ))
}

pub fn prepare(
    source: &[u8],
    draft: &[u8],
    menus: &[u8],
) -> Result<PreparedResident, Box<dyn Error>> {
    let encoding = encode_main::encode(source, draft, menus)?;
    let encoded = encoding
        .entries
        .iter()
        .map(|e| Ok((e.source_offset, hex::decode(&e.encoded_hex)?)))
        .collect::<Result<BTreeMap<_, _>, hex::FromHexError>>()?;
    let references = text_references::extract(source)?;
    let mut writes = Vec::new();
    let mut pools = BTreeMap::new();
    for table in &TABLES {
        let refs: Vec<_> = references
            .iter()
            .filter(|r| r.id.starts_with(&format!("{}-", table.name)))
            .collect();
        let (planned, budget) = pool_writes(source, table, &refs, &encoded)?;
        writes.extend(planned);
        pools.insert(table.name.to_string(), budget);
    }
    let output = resident_plan::apply(source, &writes)?;
    let mut verified_references = 0;
    for table in &TABLES {
        for index in 0..table.resolved {
            let slot = table.offset + index * 2;
            let old = word(source, slot)?
                .checked_sub(table.base)
                .ok_or("Source pointer underflow")?
                + table.offset;
            let new = word(&output, slot)?
                .checked_sub(table.base)
                .ok_or("Candidate pointer underflow")?
                + table.offset;
            let expected = encoded.get(&old).ok_or("Source pointer not encoded")?;
            let end = new
                .checked_add(expected.len())
                .ok_or("Candidate pointer overflow")?;
            if new < table.offset + table.count * 2
                || end > table.end
                || output.get(new..end) != Some(expected.as_slice())
            {
                return Err("Relocated pointer does not recover its encoded draft".into());
            }
            verified_references += 1;
        }
    }
    Ok(PreparedResident {
        plan: resident_plan::from_applied(source, &output, writes),
        encoding,
        pools,
        verified_references,
        decoder: "not_installed",
        font_loader: "not_installed",
        page_local_text: "not_relocated",
        release_eligible: false,
    })
}

#[cfg(test)]
#[path = "prepare_resident_tests.rs"]
mod tests;
