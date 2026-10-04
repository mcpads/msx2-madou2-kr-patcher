//! Assemble all inspected main-page text consumers and source-linked local strings.
use crate::{
    encode_main::EncodingReport,
    main_decoder, particles, prepare_resident,
    text_driver::{self as driver, PAGES, Page},
    text_references,
    z80_writes::CodeSource,
};
use expected_write::{ExpectedWrite, WriteIntent};
use retro_z80::{Assembler, Instruction::*, Register16};
use std::{collections::BTreeMap, error::Error};

pub struct TextInstallation {
    pub code: Vec<CodeSource>,
    pub data: Vec<ExpectedWrite>,
    pub glyph_count: u16,
    pub local_spans: usize,
}

fn data_write(
    source: &[u8],
    id: String,
    offset: usize,
    bytes: Vec<u8>,
    intent: WriteIntent,
) -> Result<ExpectedWrite, Box<dyn Error>> {
    Ok(ExpectedWrite {
        id,
        owner: "main-text".into(),
        purpose: "Source-linked text, pointer or runtime table".into(),
        offset,
        expected_original: source
            .get(offset..offset + bytes.len())
            .ok_or("Missing text region")?
            .to_vec(),
        replacement: bytes,
        intent,
    })
}
fn padding(
    source: &[u8],
    page: Page,
    start: u16,
    length: usize,
    end: u16,
) -> Result<(), Box<dyn Error>> {
    if usize::from(start) + length > usize::from(end)
        || source
            .get(page.disk(start)..page.disk(start) + length)
            .ok_or("Missing text placement")?
            .iter()
            .any(|&n| n != 0xe5)
    {
        return Err(format!(
            "Page {} text placement {start:04x}+{length} exceeds padding/end {end:04x}",
            page.number
        )
        .into());
    }
    Ok(())
}

pub fn prepare(
    source: &[u8],
    encoding: &EncodingReport,
    draft: &[u8],
    menus: &[u8],
) -> Result<TextInstallation, Box<dyn Error>> {
    let resident = prepare_resident::prepare(source, draft, menus)?;
    let mut result = TextInstallation {
        code: vec![],
        data: vec![],
        glyph_count: u16::try_from(256 + encoding.characters().count())?,
        local_spans: 0,
    };
    for write in resident.plan.writes {
        let intent = if write.name.ends_with("-text") {
            WriteIntent::Data
        } else {
            WriteIntent::Metadata
        };
        result.data.push(data_write(
            source,
            write.name,
            write.offset,
            hex::decode(write.replacement_hex)?,
            intent,
        )?);
    }
    let entries: BTreeMap<_, _> = encoding
        .entries
        .iter()
        .map(|e| (e.source_offset, e))
        .collect();
    let refs = text_references::extract(source)?;
    let (classes, pairs) = particles::tables(encoding)?;
    let common = &text_references::TABLES[0];
    let common_start = u16::try_from(common.base + common.count * 2)?;
    let common_end = u16::try_from(common.base + common.end - common.offset)?;
    let bank = &text_references::TABLES[1];
    let bank_end = u16::try_from(bank.base + bank.end - bank.offset)?;
    for page in PAGES {
        // Keep raw-pointer references until extraction has verified their original CALL/table consumers.
        let prefix = format!("page-{:02x}-", page.number);
        let mut addresses = BTreeMap::new();
        let mut pool = Vec::new();
        let pool_start = if page.number == 7 {
            0x7000
        } else {
            driver::TEXT
        };
        for reference in refs.iter().filter(|r| r.id.starts_with(&prefix)) {
            let address = if let Some(&address) = addresses.get(&reference.offset) {
                address
            } else {
                let bytes = hex::decode(&entries[&reference.offset].encoded_hex)?;
                let address = u16::try_from(usize::from(pool_start) + pool.len())?;
                pool.push(0);
                pool.extend(u16::try_from(bytes.len())?.to_le_bytes());
                pool.extend(bytes);
                addresses.insert(reference.offset, address);
                address
            };
            let suffix = reference
                .id
                .strip_prefix(&prefix)
                .ok_or("Missing page reference prefix")?;
            let pointer = if let Some(call) = suffix.strip_prefix("call-") {
                u16::from_str_radix(call, 16)? + 1
            } else {
                let (base, index) = suffix
                    .strip_prefix("table-")
                    .ok_or("Unknown local reference")?
                    .split_once('-')
                    .ok_or("Malformed table reference")?;
                u16::from_str_radix(base, 16)? + index.parse::<u16>()? * 2
            };
            if u16::from_le_bytes(source[page.disk(pointer)..page.disk(pointer) + 2].try_into()?)
                as usize
                != reference.offset - (page.number - 1) * 0x4000
            {
                return Err("Local reference pointer drift".into());
            }
            if suffix.starts_with("call-") {
                let mut code = Assembler::new();
                code.emit(LdRRImm(Register16::Hl, address));
                result.code.push(CodeSource {
                    id: reference.id.clone(),
                    disk_offset: page.disk(pointer - 1),
                    program: code.assemble(pointer - 1)?,
                });
            } else {
                result.data.push(data_write(
                    source,
                    reference.id.clone(),
                    page.disk(pointer),
                    address.to_le_bytes().to_vec(),
                    WriteIntent::Metadata,
                )?);
            }
        }
        padding(
            source,
            page,
            pool_start,
            pool.len(),
            if page.number == 7 {
                0x7800
            } else {
                driver::MODE
            },
        )?;
        result.local_spans += addresses.len();
        result.data.push(data_write(
            source,
            format!("page{}-local-text", page.number),
            page.disk(pool_start),
            pool,
            WriteIntent::Data,
        )?);
        let mut table = classes.clone();
        table.extend(&pairs);
        padding(source, page, driver::TABLE, table.len(), driver::MENU)?;
        result.data.push(data_write(
            source,
            format!("page{}-particles", page.number),
            page.disk(driver::TABLE),
            table,
            WriteIntent::Data,
        )?);
        padding(source, page, driver::MODE, 4, 0x8000)?;
        result.data.push(data_write(
            source,
            format!("page{}-text-state", page.number),
            page.disk(driver::MODE),
            vec![0, 0, 0, 3],
            WriteIntent::Data,
        )?);
        for (kind, program, end) in [
            ("text-tick", driver::tick(page)?, driver::DECODER),
            (
                "text-decoder",
                main_decoder::assemble_with_reader(
                    driver::DECODER,
                    result.glyph_count,
                    Some(page.reader()),
                )?,
                driver::INIT,
            ),
            (
                "text-init",
                driver::init(page, common_start, common_end, bank_end)?,
                driver::DRAW,
            ),
            ("text-draw", driver::draw(page)?, driver::CLASSIFY),
            (
                "ending-classify",
                particles::classify(driver::CLASSIFY, driver::TABLE, result.glyph_count)?,
                driver::SELECT,
            ),
            (
                "particle-select",
                particles::select(
                    driver::SELECT,
                    driver::TABLE + u16::try_from(classes.len())?,
                )?,
                driver::TABLE,
            ),
            ("text-menu", driver::menu(page, bank_end)?, driver::TEXT),
        ] {
            padding(source, page, program.origin(), program.bytes().len(), end)?;
            result.code.push(CodeSource {
                id: format!("page{}-{kind}", page.number),
                disk_offset: page.disk(program.origin()),
                program,
            });
        }
        for (kind, address, target, call, expected) in [
            (
                "text-tick-entry",
                page.tick(),
                driver::TICK,
                false,
                vec![0x3a, 0x9a, 0xc9],
            ),
            (
                "text-init-entry",
                page.init(),
                driver::INIT,
                true,
                vec![0x22, 0x90, 0xc9],
            ),
            (
                "text-menu-entry",
                page.menu,
                driver::MENU,
                false,
                vec![0x78, 0x21, 0x00, 0x30],
            ),
        ] {
            if source.get(page.disk(address)..page.disk(address) + expected.len())
                != Some(expected.as_slice())
            {
                return Err(format!("Page {} {kind} source changed", page.number).into());
            }
            let mut a = Assembler::new();
            a.emit(if call { Call(target) } else { Jp(target) });
            if expected.len() == 4 {
                a.emit(Nop);
            }
            result.code.push(CodeSource {
                id: format!("page{}-{kind}", page.number),
                disk_offset: page.disk(address),
                program: a.assemble(address)?,
            });
        }
    }
    Ok(result)
}
