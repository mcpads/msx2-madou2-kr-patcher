//! Original main-text grammar and kana annotation, bounded by the owning pool.
use crate::main_encoding::Token;
use std::error::Error;
use unicode_normalization::UnicodeNormalization;

pub fn parse(
    data: &[u8],
    start: usize,
    limit: usize,
) -> Result<(Vec<Token>, usize), Box<dyn Error>> {
    let raw = data
        .get(start..limit)
        .filter(|r| !r.is_empty())
        .ok_or("Invalid source text bounds")?;
    let mut position = 0;
    let mut result = Vec::new();
    while position < raw.len() {
        let code = raw[position];
        position += 1;
        match code {
            0xff => {
                result.push(Token::End);
                return Ok((result, start + position));
            }
            b'$' => {
                let command = *raw.get(position).ok_or("Truncated source command")?;
                position += 1;
                match command {
                    b'$' => result.push(Token::Glyph('$')),
                    b'@' | b'r' | b'c' | b'k' => result.push(Token::Command(command, None)),
                    b'w' | b't' => {
                        let argument = *raw.get(position).ok_or("Truncated source argument")?;
                        position += 1;
                        result.push(Token::Command(command, Some(argument)));
                    }
                    _ => return Err("Unknown source command".into()),
                }
            }
            0x20..=0x7e => result.push(Token::Glyph(code as char)),
            0xa1..=0xdf => result.push(Token::Glyph(
                char::from_u32(0xff61 + u32::from(code - 0xa1)).unwrap(),
            )),
            _ => return Err("Unknown source glyph".into()),
        }
    }
    Err("Missing source terminator".into())
}

pub fn annotate(tokens: &[Token]) -> String {
    let mut result = String::new();
    let mut run = String::new();
    let mut hiragana = true;
    for token in tokens {
        if let Token::Glyph(c) = token {
            run.push(*c);
            continue;
        }
        for c in run.nfkc() {
            result.push(if hiragana && ('\u{30a1}'..='\u{30f6}').contains(&c) {
                char::from_u32(c as u32 - 0x60).unwrap()
            } else {
                c
            });
        }
        run.clear();
        match token {
            Token::Command(code, argument) => {
                let name = match code {
                    b'w' => "WAIT_TICKS",
                    b't' => "CHARACTER_DELAY",
                    b'r' => "NEWLINE",
                    b'c' => "CLEAR_WINDOW",
                    b'k' => "WAIT_INPUT",
                    b'@' => "TOGGLE_KANA",
                    _ => unreachable!("Only original parsed commands are annotated"),
                };
                result.push('<');
                result.push_str(name);
                if let Some(value) = argument {
                    result.push_str(&format!(":{value:02X}"));
                }
                result.push('>');
                if *code == b'@' {
                    hiragana = !hiragana;
                }
            }
            Token::End => result.push_str("<END>"),
            _ => unreachable!("Only original parsed text is annotated"),
        }
    }
    result
}

pub fn word(source: &[u8], offset: usize) -> Result<usize, Box<dyn Error>> {
    let end = offset.checked_add(2).ok_or("Pointer overflow")?;
    let bytes = source.get(offset..end).ok_or("Pointer outside disk")?;
    Ok(u16::from_le_bytes(bytes.try_into()?) as usize)
}

#[cfg(test)]
#[path = "main_source_tests.rs"]
mod tests;
