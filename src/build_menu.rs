//! Candidate menu disk: bind text encoding and font generation to the same drafts.
use crate::{encode_main, font_storage, main_font, media, menu_hook, rle, storage_writes::Probe};
use serde::Serialize;
use std::{error::Error, path::Path};

#[derive(Serialize)]
struct Report {
    #[serde(flatten)]
    storage: font_storage::StoragePlan,
    encoding: encode_main::EncodingReport,
    menu_names_installed: usize,
    bdf_sha256: String,
    scope: &'static str,
}

pub fn run(args: &[String]) -> Result<Vec<u8>, Box<dyn Error>> {
    let source = media::read_source(Path::new(&args[0]))?;
    let encoding = encode_main::encode(
        &source,
        &std::fs::read(&args[1])?,
        &std::fs::read(&args[2])?,
    )?;
    let bdf = std::fs::read(&args[3])?;
    let font = rle::encode(&main_font::compile(&source, &encoding, &bdf)?);
    let menu = menu_hook::prepare(&source, &encoding)?;
    let names = menu.names;
    let storage = font_storage::prepare_with_menu(&source, &font, Probe::MainMenus, Some(menu))?;
    let disk = Path::new(&args[4]);
    if disk == Path::new(&args[5]) {
        return Err("Disk and report paths must differ".into());
    }
    let report = Report {
        storage,
        encoding,
        menu_names_installed: names,
        bdf_sha256: media::sha256(&bdf),
        scope: "Page-01 menu names only; remaining pages and general text are not translated",
    };
    let json = serde_json::to_vec_pretty(&report)?;
    if let Some(parent) = disk.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(disk, &report.storage.writes.candidate)?;
    Ok(json)
}
