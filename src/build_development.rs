//! Build source-identified cutscenes alone or together with inspected main-game text.
use crate::{
    cutscene_install, encode_main, font_storage, main_font, media, rle,
    storage_writes::{self, Probe},
    text_install,
    z80_writes::Verifier,
};
use std::{error::Error, path::Path};

pub fn run(args: &[String], integrated: bool) -> Result<Vec<u8>, Box<dyn Error>> {
    let source = media::read_source(Path::new(&args[0]))?;
    let draft = std::fs::read(&args[1])?;
    let ttf = std::fs::read(&args[2])?;
    let mut installation = cutscene_install::prepare(&source, &draft, &ttf)?;
    let credits_path = Path::new(&args[1]).with_file_name("credits.ko.json");
    let credits = std::fs::read(credits_path)?;
    installation.add_credits(&source, &credits, &ttf)?;
    let title = std::fs::read(Path::new(&args[1]).with_file_name("title.ko.json"))?;
    let title_graphics_path = Path::new(&args[1])
        .parent()
        .ok_or("Missing translation directory")?
        .join("../graphics/title/cells.json");
    let title_graphics = std::fs::read(title_graphics_path)?;
    crate::title::install(&source, &title, &title_graphics, &mut installation)?;
    let mut boot_warning_sha256 = None;
    if integrated {
        let warning = std::fs::read(Path::new(&args[3]).with_file_name("boot-warning.ko.json"))?;
        crate::boot_warning::install(
            &source,
            &warning,
            &std::fs::read(&args[5])?,
            &mut installation,
        )?;
        boot_warning_sha256 = Some(media::sha256(&warning));
    }
    let machine_code = std::mem::take(&mut installation.machine_code);
    let mut direct_labels_sha256 = None;
    let (candidate, writes, main) = if integrated {
        let main_draft = std::fs::read(&args[3])?;
        let menus = std::fs::read(&args[4])?;
        let bdf = std::fs::read(&args[5])?;
        let encoding = encode_main::encode(&source, &main_draft, &menus)?;
        let mut text = text_install::prepare(&source, &encoding, &main_draft, &menus)?;
        let labels = std::fs::read(Path::new(&args[3]).with_file_name("direct-labels.ko.json"))?;
        crate::direct_labels::install(&source, &labels, &encoding, &mut text)?;
        direct_labels_sha256 = Some(media::sha256(&labels));
        let font = rle::encode(&main_font::compile(&source, &encoding, &bdf)?);
        let storage = font_storage::prepare_with_assets(
            &source,
            &font,
            Probe::MainText,
            None,
            Some(text),
            (installation.writes.clone(), machine_code),
        )?;
        (
            storage.writes.candidate.clone(),
            serde_json::to_value(&storage)?,
            serde_json::json!({"encoding":encoding,"bdf_sha256":media::sha256(&bdf)}),
        )
    } else {
        let receipt =
            storage_writes::audit(&source, installation.writes.clone(), Verifier(machine_code))?;
        (
            receipt.candidate.clone(),
            serde_json::to_value(receipt)?,
            serde_json::Value::Null,
        )
    };
    installation.verify(&candidate)?;
    let report = serde_json::json!({"kind":"non-distribution-development",
        "source_sha256":media::sha256(&source),"artifact_sha256":media::sha256(&candidate),
        "size_bytes":candidate.len(),"cutscenes":installation.draft_report,
        "direct_labels_sha256":direct_labels_sha256,
        "boot_warning_sha256":boot_warning_sha256,
        "title_sha256":media::sha256(&title),
        "title_graphics_sha256":media::sha256(&title_graphics),
        "ttf_sha256":media::sha256(&ttf),"writes":writes,"main":main,
        "cutscene_final_references_and_fonts_verified":true,
        "main_text_inserted":integrated,"runtime":"pending","human_review":"pending",
        "release_eligible":false});
    let json = serde_json::to_vec_pretty(&report)?;
    let output = Path::new(&args[args.len() - 2]);
    if output == Path::new(args.last().ok_or("Missing report path")?) {
        return Err("Disk and report paths must differ".into());
    }
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, candidate)?;
    Ok(json)
}
