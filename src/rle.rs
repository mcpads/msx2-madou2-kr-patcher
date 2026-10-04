//! Packet format used by the identified game's bitmap reader.
pub fn encode(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut position = 0;
    while position < data.len() {
        let count = run(data, position, 127);
        if count >= 3 {
            result.extend([128 + count as u8, data[position]]);
            position += count;
            continue;
        }
        let start = position;
        position += count;
        while position < data.len() && position - start < 127 {
            let count = run(data, position, 3);
            if count >= 3 {
                break;
            }
            position += count.min(127 - (position - start));
        }
        result.push((position - start) as u8);
        result.extend_from_slice(&data[start..position]);
    }
    result.push(0);
    result
}

fn run(data: &[u8], position: usize, limit: usize) -> usize {
    let mut count = 1;
    while position + count < data.len() && count < limit && data[position + count] == data[position]
    {
        count += 1;
    }
    count
}

pub fn decode(data: &[u8], expected: usize) -> Result<Vec<u8>, &'static str> {
    let (result, consumed) = decode_prefix(data, expected)?;
    if consumed != data.len() {
        return Err("RLE trailing bytes");
    }
    Ok(result)
}

pub fn decode_prefix(data: &[u8], expected: usize) -> Result<(Vec<u8>, usize), &'static str> {
    let mut result = Vec::new();
    let mut cursor = 0;
    while let Some(&control) = data.get(cursor) {
        cursor += 1;
        if control == 0 {
            return if result.len() == expected {
                Ok((result, cursor))
            } else {
                Err("RLE size mismatch or trailing bytes")
            };
        }
        let count = packet_count(control);
        if count > expected.saturating_sub(result.len()) {
            return Err("RLE output exceeds declared size");
        }
        if control & 128 != 0 {
            let value = *data.get(cursor).ok_or("Truncated RLE repeat")?;
            cursor += 1;
            result.resize(result.len() + count, value);
        } else {
            let literal = data
                .get(cursor..cursor + count)
                .ok_or("Truncated RLE literal")?;
            result.extend_from_slice(literal);
            cursor += count;
        }
    }
    Err("Missing RLE terminator")
}

pub fn packet_count(control: u8) -> usize {
    match control & 127 {
        0 => 256,
        n => n as usize,
    }
}

#[cfg(test)]
#[path = "rle_tests.rs"]
mod tests;
