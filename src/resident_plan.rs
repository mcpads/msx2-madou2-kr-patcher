//! Verify analysis-produced data writes against project-owned resident regions.
//! This stage does not certify translation semantics or install a game decoder.
use crate::{media::sha256, text_references::TABLES};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, error::Error, ops::Range};

#[derive(Deserialize, Serialize)]
pub struct AnalysisPlan {
    kind: String,
    source_sha256: String,
    candidate_disk_sha256: String,
    candidate_disk_size: usize,
    changed_bytes: usize,
    pub writes: Vec<AnalysisWrite>,
}

#[derive(Deserialize, Serialize)]
pub struct AnalysisWrite {
    pub name: String,
    pub offset: usize,
    pub expected_hex: String,
    pub replacement_hex: String,
}

pub fn apply(baseline: &[u8], writes: &[AnalysisWrite]) -> Result<Vec<u8>, Box<dyn Error>> {
    compile_writes(baseline, writes, regions())?
        .apply(baseline, None)
        .map_err(Into::into)
}

pub fn from_applied(baseline: &[u8], output: &[u8], writes: Vec<AnalysisWrite>) -> AnalysisPlan {
    AnalysisPlan {
        kind: "static-resident-text-relocation".into(),
        source_sha256: sha256(baseline),
        candidate_disk_sha256: sha256(output),
        candidate_disk_size: output.len(),
        changed_bytes: baseline.iter().zip(output).filter(|(a, b)| a != b).count(),
        writes,
    }
}

#[derive(Serialize)]
pub struct Receipt {
    kind: &'static str,
    source_sha256: String,
    candidate_disk_sha256: String,
    candidate_disk_size: usize,
    changed_bytes: usize,
    verified_writes: usize,
    pub verified_pointers: usize,
    translation_semantics_verified: bool,
    game_decoder_installed: bool,
    release_eligible: bool,
}

fn regions() -> Vec<ImageRegion> {
    let mut regions = Vec::new();
    for table in &TABLES {
        for index in 0..table.resolved {
            let start = table.offset + index * 2;
            regions.push(ImageRegion {
                id: format!("{}-{index:03}", table.name),
                range: start..start + 2,
                kind: RegionKind::Metadata,
                reason: "Resolved resident text pointer".into(),
            });
        }
        regions.push(ImageRegion {
            id: format!("{}-text", table.name),
            range: table.offset + table.count * 2..table.end,
            kind: RegionKind::Data,
            reason: "Source resident text pool".into(),
        });
    }
    regions
}

fn compile_writes(
    baseline: &[u8],
    writes: &[AnalysisWrite],
    regions: Vec<ImageRegion>,
) -> Result<WritePlan, Box<dyn Error>> {
    let mut plan = WritePlan::new();
    let mut ids = BTreeSet::new();
    for write in writes {
        let region = regions
            .iter()
            .find(|r| r.id == write.name)
            .ok_or("Unknown writer; no owned region")?;
        let expected_original = hex::decode(&write.expected_hex)?;
        let replacement = hex::decode(&write.replacement_hex)?;
        if !ids.insert(&write.name)
            || write.offset != region.range.start
            || replacement.len() != region.range.len()
            || expected_original.len() != replacement.len()
        {
            return Err("Duplicate, incomplete or misplaced resident writer".into());
        }
        let intent = match region.kind {
            RegionKind::Data => WriteIntent::Data,
            RegionKind::Metadata => WriteIntent::Metadata,
            _ => return Err("Resident writes cannot contain machine code".into()),
        };
        plan = plan.write(ExpectedWrite {
            id: write.name.clone(),
            owner: "resident-text".into(),
            purpose: "Candidate resident translation relocation".into(),
            offset: write.offset,
            expected_original,
            replacement,
            intent,
        });
    }
    if ids.len() != regions.len() {
        return Err("Missing resident writer".into());
    }
    let mut cursor = 0;
    let mut sorted = regions;
    sorted.sort_by_key(|r| r.range.start);
    for region in sorted {
        if cursor < region.range.start {
            plan = plan.region(protected(cursor..region.range.start));
        }
        cursor = region.range.end;
        plan = plan.region(region);
    }
    if cursor < baseline.len() {
        plan = plan.region(protected(cursor..baseline.len()));
    }
    Ok(plan)
}

fn protected(range: Range<usize>) -> ImageRegion {
    ImageRegion {
        id: format!("protected-{:x}", range.start),
        range,
        kind: RegionKind::Protected,
        reason: "Outside resolved resident pointers and text pools".into(),
    }
}

pub fn verify(baseline: &[u8], analysis: AnalysisPlan) -> Result<Receipt, Box<dyn Error>> {
    if analysis.kind != "static-resident-text-relocation"
        || analysis.source_sha256 != sha256(baseline)
        || analysis.candidate_disk_size != baseline.len()
    {
        return Err("Analysis input identity or kind mismatch".into());
    }
    let plan = compile_writes(baseline, &analysis.writes, regions())?;
    let output = plan.apply(baseline, None)?;
    let changed_bytes = baseline.iter().zip(&output).filter(|(a, b)| a != b).count();
    let candidate_hash = sha256(&output);
    if candidate_hash != analysis.candidate_disk_sha256 || changed_bytes != analysis.changed_bytes {
        return Err("Rust output differs from analysis candidate".into());
    }
    let mut verified_pointers = 0;
    for table in &TABLES {
        for index in 0..table.resolved {
            let offset = table.offset + index * 2;
            let address = u16::from_le_bytes(output[offset..offset + 2].try_into()?) as usize;
            let target = address
                .checked_sub(table.base)
                .and_then(|n| n.checked_add(table.offset))
                .ok_or("Resident pointer underflow")?;
            if !(table.offset + table.count * 2..table.end).contains(&target) {
                return Err("Resident pointer escapes its text pool".into());
            }
            verified_pointers += 1;
        }
    }
    Ok(Receipt {
        kind: "rust-resident-write-verification",
        source_sha256: sha256(baseline),
        candidate_disk_sha256: candidate_hash,
        candidate_disk_size: output.len(),
        changed_bytes,
        verified_writes: analysis.writes.len(),
        verified_pointers,
        translation_semantics_verified: false,
        game_decoder_installed: false,
        release_eligible: false,
    })
}

#[cfg(test)]
#[path = "resident_plan_tests.rs"]
mod tests;
