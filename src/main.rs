mod boot_warning;
mod build_development;
mod build_menu;
mod build_text;
mod credits;
mod cutscene_draft;
mod cutscene_font;
mod cutscene_install;
mod cutscene_source;
mod direct_labels;
mod draft_policy;
mod encode_main;
mod font_loader;
mod font_storage;
mod glyph_expand;
mod glyph_hook;
mod loader_restore;
mod main_decoder;
mod main_encoding;
mod main_font;
mod main_source;
mod media;
mod menu_draft;
mod menu_hook;
mod page_restore;
mod particles;
mod prepare_resident;
mod resident_plan;
mod rle;
mod storage_writes;
mod text_driver;
mod text_install;
mod text_references;
mod title;
mod z80_writes;

use std::{error::Error, path::PathBuf};

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let draft_command = args
        .first()
        .is_some_and(|s| matches!(s.as_str(), "encode-main" | "prepare-resident"));
    let argument_count = if args.first().is_some_and(|s| s == "build-development") {
        9
    } else if args.first().is_some_and(|s| s == "decoder-reader") {
        5
    } else if args
        .first()
        .is_some_and(|s| matches!(s.as_str(), "build-menu-probe" | "build-text-probe"))
    {
        7
    } else if args.first().is_some_and(|s| {
        matches!(
            s.as_str(),
            "compile-font" | "prepare-particles" | "build-cutscenes"
        )
    }) {
        6
    } else if args.first().is_some_and(|s| {
        matches!(
            s.as_str(),
            "page-restore" | "font-loader" | "inspect-cutscenes"
        )
    }) {
        3
    } else if draft_command
        || args.first().is_some_and(|s| {
            matches!(
                s.as_str(),
                "build-storage-probe" | "build-renderer-probe" | "compile-cutscene-fonts"
            )
        })
    {
        5
    } else {
        4
    };
    if args.len() != argument_count {
        return Err("Usage: build-development <source-root> <cutscene-draft> <ttf> <main-draft> <menu-draft> <bdf> <output-disk> <output-json> | build-cutscenes <source-root> <draft-json> <ttf> <output-disk> <output-json> | compile-cutscene-fonts <source-root> <draft-json> <ttf> <output-json> | prepare-cutscenes <source-root> <draft-json> <output-json> | inspect-cutscenes <source-root> <output-json> | prepare-particles <source-root> <draft-json> <menu-json> <origin-hex> <output-json> | decoder-reader <origin-hex> <glyph-limit> <reader-address-hex> <output> | build-menu-probe/build-text-probe <source-root> <draft-json> <menu-json> <bdf> <output-disk> <output-json> | compile-font <source-root> <draft-json> <menu-json> <bdf> <output-rle> | decoder/glyph-expand <origin-hex> <glyph-limit> <output> | page-restore/font-loader <origin-hex> <output> | loader-restore <origin-hex> <stream-address-hex> <output> | prepare-storage <source-root> <font-rle> <output-json> | build-storage-probe/build-renderer-probe <source-root> <font-rle> <output-disk> <output-json> | verify-resident <source-root> <analysis-json> <receipt-json> | encode-main/prepare-resident <source-root> <draft-json> <menu-json> <output-json>".into());
    }
    let output = PathBuf::from(args.last().unwrap());
    let bytes = match args[0].as_str() {
        "build-development" => build_development::run(&args[1..], true)?,
        "build-cutscenes" => build_development::run(&args[1..], false)?,
        "compile-cutscene-fonts" => {
            let source = media::read_source(std::path::Path::new(&args[1]))?;
            let prepared = cutscene_draft::prepare(&source, &std::fs::read(&args[2])?)?;
            serde_json::to_vec_pretty(&cutscene_font::compile(
                &prepared,
                &std::fs::read(&args[3])?,
            )?)?
        }
        "prepare-cutscenes" => {
            let source = media::read_source(std::path::Path::new(&args[1]))?;
            serde_json::to_vec_pretty(&cutscene_draft::prepare(
                &source,
                &std::fs::read(&args[2])?,
            )?)?
        }
        "inspect-cutscenes" => {
            let source = media::read_source(std::path::Path::new(&args[1]))?;
            serde_json::to_vec_pretty(&cutscene_source::read(&source)?)?
        }
        "build-text-probe" => build_text::run(&args[1..])?,
        "build-menu-probe" => build_menu::run(&args[1..])?,
        "prepare-particles" => {
            let source = media::read_source(std::path::Path::new(&args[1]))?;
            let encoding = encode_main::encode(
                &source,
                &std::fs::read(&args[2])?,
                &std::fs::read(&args[3])?,
            )?;
            let origin = u16::from_str_radix(args[4].trim_start_matches("0x"), 16)?;
            let prepared = particles::prepare(&encoding, origin)?;
            serde_json::to_vec_pretty(
                &serde_json::json!({"encoding": encoding, "particles": prepared}),
            )?
        }
        "compile-font" => {
            let source = media::read_source(std::path::Path::new(&args[1]))?;
            let encoding = encode_main::encode(
                &source,
                &std::fs::read(&args[2])?,
                &std::fs::read(&args[3])?,
            )?;
            rle::encode(&main_font::compile(
                &source,
                &encoding,
                &std::fs::read(&args[4])?,
            )?)
        }
        "glyph-expand" => {
            let origin = u16::from_str_radix(args[1].trim_start_matches("0x"), 16)?;
            glyph_expand::assemble(origin, args[2].parse()?)?.into_bytes()
        }
        "font-loader" => {
            let origin = u16::from_str_radix(args[1].trim_start_matches("0x"), 16)?;
            font_loader::assemble(origin)?.into_bytes()
        }
        "prepare-storage" => {
            let source = media::read_source(std::path::Path::new(&args[1]))?;
            let report = font_storage::prepare(
                &source,
                &std::fs::read(&args[2])?,
                storage_writes::Probe::Storage,
            )?;
            serde_json::to_vec_pretty(&report)?
        }
        "build-storage-probe" | "build-renderer-probe" => {
            let source = media::read_source(std::path::Path::new(&args[1]))?;
            let probe = if args[0] == "build-renderer-probe" {
                storage_writes::Probe::PageOneRenderer
            } else {
                storage_writes::Probe::FontLoad
            };
            let report = font_storage::prepare(&source, &std::fs::read(&args[2])?, probe)?;
            let disk_path = PathBuf::from(&args[3]);
            if disk_path == output {
                return Err("Disk and report paths must differ".into());
            }
            if let Some(parent) = disk_path.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(disk_path, &report.writes.candidate)?;
            serde_json::to_vec_pretty(&report)?
        }
        "loader-restore" => {
            let origin = u16::from_str_radix(args[1].trim_start_matches("0x"), 16)?;
            let stream = u16::from_str_radix(args[2].trim_start_matches("0x"), 16)?;
            loader_restore::assemble(origin, stream)?.into_bytes()
        }
        "page-restore" => {
            let origin = u16::from_str_radix(args[1].trim_start_matches("0x"), 16)?;
            page_restore::assemble(origin)?.into_bytes()
        }
        "decoder-reader" => {
            let origin = u16::from_str_radix(args[1].trim_start_matches("0x"), 16)?;
            let reader = u16::from_str_radix(args[3].trim_start_matches("0x"), 16)?;
            main_decoder::assemble_with_reader(origin, args[2].parse()?, Some(reader))?.into_bytes()
        }
        "decoder" => {
            let origin = u16::from_str_radix(args[1].trim_start_matches("0x"), 16)?;
            let glyph_limit: u16 = args[2].parse()?;
            main_decoder::assemble(origin, glyph_limit)?.into_bytes()
        }
        "verify-resident" => {
            let source = media::read_source(std::path::Path::new(&args[1]))?;
            let analysis = serde_json::from_slice(&std::fs::read(&args[2])?)?;
            let receipt = resident_plan::verify(&source, analysis)?;
            serde_json::to_vec_pretty(&receipt)?
        }
        "encode-main" => {
            let source = media::read_source(std::path::Path::new(&args[1]))?;
            let report = encode_main::encode(
                &source,
                &std::fs::read(&args[2])?,
                &std::fs::read(&args[3])?,
            )?;
            serde_json::to_vec_pretty(&report)?
        }
        "prepare-resident" => {
            let source = media::read_source(std::path::Path::new(&args[1]))?;
            let report = prepare_resident::prepare(
                &source,
                &std::fs::read(&args[2])?,
                &std::fs::read(&args[3])?,
            )?;
            serde_json::to_vec_pretty(&report)?
        }
        _ => return Err("Unknown command".into()),
    };
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output, &bytes)?;
    println!("{} bytes: {}", bytes.len(), output.display());
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
