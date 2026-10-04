//! Candidate wire encoding shared by the Rust insertion pipeline.
use std::{collections::BTreeMap, error::Error};

pub const PARTICLES: [(&str, &str); 4] = [
    ("topic", "은는"),
    ("subject", "이가"),
    ("object", "을를"),
    ("with", "과와"),
];

#[derive(Debug, PartialEq)]
pub(crate) enum Token {
    Glyph(char),
    Command(u8, Option<u8>),
    Particle(u8),
    End,
}

pub(crate) fn tokenize(text: &str) -> Result<Vec<Token>, Box<dyn Error>> {
    let mut chars = text.chars();
    let mut result = Vec::new();
    let mut ended = false;
    while let Some(character) = chars.next() {
        if ended {
            return Err("Content after terminator".into());
        }
        if character != '<' {
            if character == '>' || character.is_control() {
                return Err("Malformed literal".into());
            }
            result.push(Token::Glyph(character));
            continue;
        }
        let mut body = String::new();
        loop {
            match chars.next() {
                Some('>') => break,
                None | Some('<') => return Err("Malformed token".into()),
                Some(c) => body.push(c),
            }
        }
        let token = match body.as_str() {
            "END" => {
                ended = true;
                Token::End
            }
            "NEWLINE" => Token::Command(b'r', None),
            "CLEAR_WINDOW" => Token::Command(b'c', None),
            "WAIT_INPUT" => Token::Command(b'k', None),
            _ => {
                let (name, argument) = body.split_once(':').ok_or("Unknown control")?;
                if name == "JOSA" {
                    let index = PARTICLES
                        .iter()
                        .position(|(kind, _)| *kind == argument)
                        .ok_or("Unknown particle")?;
                    Token::Particle(index as u8 + 1)
                } else {
                    let code = match name {
                        "WAIT_TICKS" => b'w',
                        "CHARACTER_DELAY" => b't',
                        _ => return Err("Unknown authoring control".into()),
                    };
                    if argument.len() != 2
                        || !argument
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
                    {
                        return Err("Control argument must be two uppercase hex digits".into());
                    }
                    Token::Command(code, Some(u8::from_str_radix(argument, 16)?))
                }
            }
        };
        result.push(token);
    }
    if !ended {
        return Err("Missing terminator".into());
    }
    Ok(result)
}

/// Entries remain in glyph-index order: the first entry is font index 256.
pub struct Mapping(pub Vec<(char, Vec<u8>)>);

impl Mapping {
    pub fn from_texts<'a>(
        texts: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut counts = BTreeMap::<char, usize>::new();
        for text in texts {
            for token in tokenize(text)? {
                match token {
                    Token::Glyph(c) if c as u32 > 127 => *counts.entry(c).or_default() += 1,
                    Token::Particle(selector) => {
                        for c in PARTICLES[selector as usize - 1].1.chars() {
                            counts.entry(c).or_default();
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut ranked: Vec<_> = counts.into_iter().collect();
        ranked.sort_by_key(|&(c, count)| (std::cmp::Reverse(count), c));
        let singles: Vec<u8> = (5..0x20).chain(0x7f..0xa1).chain(0xe0..0xff).collect();
        if ranked.len() > singles.len() + 3 * 256 {
            return Err("Candidate character code capacity exceeded".into());
        }
        Ok(Self(
            ranked
                .into_iter()
                .enumerate()
                .map(|(index, (c, _))| {
                    let bytes = if index < singles.len() {
                        vec![singles[index]]
                    } else {
                        let n = index - singles.len();
                        vec![(1 + n / 256) as u8, (n % 256) as u8]
                    };
                    (c, bytes)
                })
                .collect(),
        ))
    }

    pub fn encode(&self, text: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        let lookup: BTreeMap<_, _> = self.0.iter().map(|(c, bytes)| (*c, bytes)).collect();
        let mut bytes = Vec::new();
        for token in tokenize(text)? {
            match token {
                Token::Glyph('$') => bytes.extend(b"$$"),
                Token::Glyph(c) if (' '..='~').contains(&c) => bytes.push(c as u8),
                Token::Glyph(c) => bytes.extend(*lookup.get(&c).ok_or("Unmapped character")?),
                Token::Command(code, argument) => {
                    bytes.extend([b'$', code]);
                    if let Some(value) = argument {
                        bytes.push(value);
                    }
                }
                Token::Particle(selector) => bytes.extend([4, selector]),
                Token::End => bytes.push(0xff),
            }
        }
        Ok(bytes)
    }
}

#[cfg(test)]
#[path = "main_encoding_tests.rs"]
mod tests;
