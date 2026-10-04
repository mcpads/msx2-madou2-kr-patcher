//! Validate authored flow and numeric payloads against original disk bytes.
use crate::main_encoding::{Token, tokenize};
use std::error::Error;
use unicode_normalization::UnicodeNormalization;

fn controls(tokens: &[Token]) -> Vec<&Token> {
    tokens
        .iter()
        .filter(|t| {
            matches!(t, Token::Command(..) | Token::End) && !matches!(t, Token::Command(b'@', _))
        })
        .collect()
}

fn numbers(tokens: &[Token]) -> Vec<String> {
    // Match the established [0-9]+g? policy after removing control/particle tokens.
    let literal: Vec<char> = tokens
        .iter()
        .filter_map(|t| match t {
            Token::Glyph(c) => Some(*c),
            _ => None,
        })
        .collect();
    let mut result = Vec::new();
    let mut position = 0;
    while position < literal.len() {
        if !literal[position].is_ascii_digit() {
            position += 1;
            continue;
        }
        let start = position;
        while position < literal.len() && literal[position].is_ascii_digit() {
            position += 1;
        }
        if literal.get(position) == Some(&'g') {
            position += 1;
        }
        result.push(literal[start..position].iter().collect());
    }
    result
}

pub fn validate(raw: &[u8], text: &str) -> Result<(), Box<dyn Error>> {
    if !text.nfc().eq(text.chars()) {
        return Err("Draft text is not NFC".into());
    }
    let translated = tokenize(text)?;
    for token in &translated {
        if let Token::Glyph(c) = token
            && (*c == '$'
                || ('\u{3040}'..='\u{30ff}').contains(c)
                || ('\u{4e00}'..='\u{9fff}').contains(c))
        {
            return Err("Implicit control or source-language character in draft".into());
        }
    }
    let (source, end) = crate::main_source::parse(raw, 0, raw.len())?;
    if end != raw.len() {
        return Err("Source content after terminator".into());
    }
    if controls(&source) != controls(&translated) {
        return Err("Source flow, delay or terminator changed".into());
    }
    if numbers(&source) != numbers(&translated) {
        return Err("Literal numeric payload changed".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "draft_policy_tests.rs"]
mod tests;
