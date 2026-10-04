//! Main text probe: one source/draft/font lineage and one audited write plan.
use crate::{
    encode_main, font_storage, main_font, media, rle, storage_writes::Probe, text_install,
};
use serde::Serialize;
use std::{error::Error, path::Path};

#[derive(Serialize)]
struct Report {
    #[serde(flatten)]
    storage: font_storage::StoragePlan,
    encoding: encode_main::EncodingReport,
    local_spans_installed: usize,
    bdf_sha256: String,
    scope: &'static str,
}

pub fn run(args: &[String]) -> Result<Vec<u8>, Box<dyn Error>> {
    let source = media::read_source(Path::new(&args[0]))?;
    let draft = std::fs::read(&args[1])?;
    let menus = std::fs::read(&args[2])?;
    let bdf = std::fs::read(&args[3])?;
    let encoding = encode_main::encode(&source, &draft, &menus)?;
    let text = text_install::prepare(&source, &encoding, &draft, &menus)?;
    let local_spans_installed = text.local_spans;
    let font = rle::encode(&main_font::compile(&source, &encoding, &bdf)?);
    let storage =
        font_storage::prepare_with_text(&source, &font, Probe::MainText, None, Some(text))?;
    let report = Report {
        storage,
        encoding,
        local_spans_installed,
        bdf_sha256: media::sha256(&bdf),
        scope: "Inspected resident and page01-07 text. Uninspected consumers, cutscenes and full-game runtime remain unverified",
    };
    let json = serde_json::to_vec_pretty(&report)?;
    let disk = Path::new(&args[4]);
    if disk == Path::new(&args[5]) {
        return Err("Disk and report paths must differ".into());
    }
    if let Some(parent) = disk.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(disk, &report.storage.writes.candidate)?;
    Ok(json)
}
