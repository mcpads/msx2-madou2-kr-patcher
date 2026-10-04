//! Lossless co-storage candidate. Does not install writes into a disk.
use crate::{loader_restore, media::sha256, rle};
use serde::Serialize;
use std::error::Error;

pub struct PackedPage {
    pub bytes: Vec<u8>,
    pub stream_offset: usize,
    pub minimum_unread_gap: usize,
}

pub fn pack(original: &[u8], prefix: &[u8]) -> Result<PackedPage, &'static str> {
    let mut stream = rle::encode(original);
    stream.pop(); // Output-size termination avoids an unread final terminator.
    let start = original
        .len()
        .checked_sub(stream.len())
        .ok_or("Page does not fit")?;
    if prefix.len() > start || original.is_empty() {
        return Err("Page does not fit");
    }
    let mut bytes = vec![0; original.len()];
    bytes[..prefix.len()].copy_from_slice(prefix);
    bytes[start..].copy_from_slice(&stream);
    let mut restored = bytes.clone();
    let mut reader = start;
    let mut writer = 0;
    let mut minimum_unread_gap = start;
    while writer < original.len() {
        let control = *restored.get(reader).ok_or("Packet reader escaped page")?;
        reader += 1;
        if control == 0 {
            return Err("Premature packet terminator");
        }
        let count = rle::packet_count(control);
        if count > original.len() - writer {
            return Err("Packet output escaped page");
        }
        let repeat = control & 128 != 0;
        let mut value = 0;
        if repeat {
            value = *restored.get(reader).ok_or("Truncated repeat packet")?;
            reader += 1;
        }
        for _ in 0..count {
            if !repeat {
                value = *restored.get(reader).ok_or("Truncated literal packet")?;
                reader += 1;
            }
            if writer >= reader {
                return Err("Output overwrites unread packet input");
            }
            restored[writer] = value;
            writer += 1;
            minimum_unread_gap = minimum_unread_gap.min(reader - writer);
        }
    }
    if reader != restored.len() || restored != original {
        return Err("Complete original page not restored");
    }
    Ok(PackedPage {
        bytes,
        stream_offset: start,
        minimum_unread_gap,
    })
}

#[derive(Serialize)]
pub struct StoragePlan {
    #[serde(flatten)]
    pub writes: crate::storage_writes::Receipt,
    source_sha256: String,
    original_page_sha256: String,
    font_rle_sha256: String,
    font_raster_sha256: String,
    packed_page_sha256: String,
    packed_page_hex: String,
    stream_address: u16,
    unused_bytes: usize,
    minimum_unread_gap: usize,
    hook_origin: u16,
    hook_hex: String,
    hook_sha256: String,
    source_disk_modified: bool,
    game_runtime_verified: bool,
    font_input_derived_from_translation: bool,
    font_entry_installed: bool,
    main_font_renderer_pages: Vec<u8>,
    main_text_inserted: bool,
}

pub fn prepare(
    source: &[u8],
    font: &[u8],
    probe: crate::storage_writes::Probe,
) -> Result<StoragePlan, Box<dyn Error>> {
    prepare_with_menu(source, font, probe, None)
}

pub fn prepare_with_menu(
    source: &[u8],
    font: &[u8],
    probe: crate::storage_writes::Probe,
    menu: Option<crate::menu_hook::MenuHook>,
) -> Result<StoragePlan, Box<dyn Error>> {
    prepare_with_text(source, font, probe, menu, None)
}

pub fn prepare_with_text(
    source: &[u8],
    font: &[u8],
    probe: crate::storage_writes::Probe,
    menu: Option<crate::menu_hook::MenuHook>,
    text: Option<crate::text_install::TextInstallation>,
) -> Result<StoragePlan, Box<dyn Error>> {
    prepare_with_assets(source, font, probe, menu, text, (Vec::new(), Vec::new()))
}

pub fn prepare_with_assets(
    source: &[u8],
    font: &[u8],
    probe: crate::storage_writes::Probe,
    menu: Option<crate::menu_hook::MenuHook>,
    text: Option<crate::text_install::TextInstallation>,
    additional: (
        Vec<expected_write::ExpectedWrite>,
        Vec<crate::z80_writes::CodeSource>,
    ),
) -> Result<StoragePlan, Box<dyn Error>> {
    let menu_installed = menu.is_some() || text.is_some();
    let raster = rle::decode(font, 8192)?;
    let original = source.get(0x84000..0x88000).ok_or("Missing source page")?;
    let packed = pack(original, font)?;
    let stream_address = 0x8000 + u16::try_from(packed.stream_offset)?;
    let hook = loader_restore::assemble(0x1fb0, stream_address)?.into_bytes();
    if hook.len() > 0x50
        || source
            .get(0xb1eb0..0xb1eb0 + hook.len())
            .ok_or("Missing loader padding")?
            .iter()
            .any(|&n| n != 0)
    {
        return Err("Hook exceeds inspected loader padding".into());
    }
    Ok(StoragePlan {
        writes: crate::storage_writes::verify_with_menu(
            source,
            &packed.bytes,
            stream_address,
            probe,
            menu,
            text,
            additional,
        )?,
        source_sha256: sha256(source),
        original_page_sha256: sha256(original),
        font_rle_sha256: sha256(font),
        font_raster_sha256: sha256(&raster),
        packed_page_sha256: sha256(&packed.bytes),
        packed_page_hex: hex::encode(&packed.bytes),
        stream_address,
        unused_bytes: packed.stream_offset - font.len(),
        minimum_unread_gap: packed.minimum_unread_gap,
        hook_origin: 0x1fb0,
        hook_sha256: sha256(&hook),
        hook_hex: hex::encode(hook),
        source_disk_modified: false,
        game_runtime_verified: false,
        font_input_derived_from_translation: menu_installed,
        font_entry_installed: probe != crate::storage_writes::Probe::Storage,
        main_font_renderer_pages: if matches!(
            probe,
            crate::storage_writes::Probe::MainMenus | crate::storage_writes::Probe::MainText
        ) {
            (1..=7).collect()
        } else if probe == crate::storage_writes::Probe::PageOneRenderer {
            vec![1]
        } else {
            vec![]
        },
        main_text_inserted: menu_installed,
    })
}

#[cfg(test)]
#[path = "font_storage_tests.rs"]
mod tests;
