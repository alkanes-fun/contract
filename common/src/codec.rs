


use anyhow::{anyhow, Result};

pub fn pack_u128s(vals: &[u128]) -> Vec<u8> {
    let mut out = Vec::with_capacity(vals.len() * 16);
    for v in vals {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

pub fn unpack_u128s(data: &[u8]) -> Result<Vec<u128>> {
    if data.len() % 16 != 0 {
        return Err(anyhow!("data not multiple of 16 bytes"));
    }
    Ok(data
        .chunks_exact(16)
        .map(|c| u128::from_le_bytes(c.try_into().unwrap()))
        .collect())
}





pub fn encode_string(s: &str) -> Vec<u128> {
    let bytes = s.as_bytes();
    let mut out: Vec<u128> = bytes
        .chunks(16)
        .map(|chunk| {
            let mut buf = [0u8; 16];
            buf[..chunk.len()].copy_from_slice(chunk);
            u128::from_le_bytes(buf)
        })
        .collect();
    if bytes.len() % 16 == 0 {
        out.push(0);
    }
    out
}
