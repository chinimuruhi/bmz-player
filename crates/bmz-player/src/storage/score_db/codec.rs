use super::*;

pub fn encode_beatoraja_ghost(ghost: &[u8]) -> Result<String> {
    if ghost.is_empty() {
        return Ok(String::new());
    }

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(ghost)?;
    Ok(URL_SAFE.encode(encoder.finish()?))
}

pub fn decode_beatoraja_ghost(encoded: &str, total_notes: u32) -> Result<Vec<u8>> {
    let expected_len = total_notes as usize;
    if encoded.is_empty() {
        return Ok(vec![4; expected_len]);
    }

    let compressed = URL_SAFE.decode(encoded)?;
    let mut decoder = GzDecoder::new(compressed.as_slice());
    let mut decoded = Vec::with_capacity(expected_len);
    decoder.read_to_end(&mut decoded)?;
    if decoded.len() < expected_len {
        decoded.resize(expected_len, 4);
    } else if decoded.len() > expected_len {
        decoded.truncate(expected_len);
    }
    Ok(decoded)
}
