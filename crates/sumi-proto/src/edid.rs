//! Minimal EDID 1.x base-block parsing: enough to identify a monitor.

use serde::Serialize;

pub const BLOCK_LEN: usize = 128;
const HEADER: [u8; 8] = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Edid {
    /// Three-letter PNP manufacturer id ("AUS" for ASUS).
    pub manufacturer: String,
    pub product_code: u16,
    /// Display product name descriptor (tag 0xFC), if present.
    pub name: Option<String>,
    /// Serial descriptor (tag 0xFF). Kept to tell identical monitors apart;
    /// never log or persist it in shareable files.
    #[serde(skip)]
    pub serial: Option<String>,
    pub year: u16,
    pub week: u8,
}

fn descriptor_text(d: &[u8]) -> String {
    d.iter()
        .take_while(|&&c| c != 0x0A && c != 0x00)
        .map(|&c| c as char)
        .collect::<String>()
        .trim()
        .to_string()
}

/// Parse a 128-byte base block. Returns `None` for a bad header or checksum.
pub fn parse(block: &[u8]) -> Option<Edid> {
    if block.len() < BLOCK_LEN || block[..8] != HEADER {
        return None;
    }
    if block[..BLOCK_LEN].iter().fold(0u8, |a, &b| a.wrapping_add(b)) != 0 {
        return None;
    }
    let m = u16::from_be_bytes([block[8], block[9]]);
    let letter = |shift: u16| (b'A' - 1 + ((m >> shift) & 0x1F) as u8) as char;
    let manufacturer = [letter(10), letter(5), letter(0)].iter().collect();
    let mut name = None;
    let mut serial = None;
    for off in [54usize, 72, 90, 108] {
        let d = &block[off..off + 18];
        if d[0] == 0 && d[1] == 0 {
            match d[3] {
                0xFC => name = Some(descriptor_text(&d[5..])),
                0xFF => serial = Some(descriptor_text(&d[5..])),
                _ => {}
            }
        }
    }
    Some(Edid {
        manufacturer,
        product_code: u16::from_le_bytes([block[10], block[11]]),
        name,
        serial,
        year: 1990 + block[17] as u16,
        week: block[16],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(mfg: [u8; 2], name: &str) -> Vec<u8> {
        let mut b = vec![0u8; 128];
        b[..8].copy_from_slice(&HEADER);
        b[8..10].copy_from_slice(&mfg);
        b[10..12].copy_from_slice(&0x32A1u16.to_le_bytes());
        b[16] = 17;
        b[17] = 35;
        b[54 + 3] = 0xFC;
        for (i, c) in name.bytes().chain([0x0A]).enumerate() {
            b[54 + 5 + i] = c;
        }
        let sum = b.iter().fold(0u8, |a, &x| a.wrapping_add(x));
        b[127] = 0u8.wrapping_sub(sum);
        b
    }

    #[test]
    fn decodes_manufacturer_and_name() {
        // "AUS" = 00001 10101 10011 → 0x06B3.
        let e = parse(&block([0x06, 0xB3], "PG32UCDM3")).unwrap();
        assert_eq!(e.manufacturer, "AUS");
        assert_eq!(e.name.as_deref(), Some("PG32UCDM3"));
        assert_eq!(e.product_code, 0x32A1);
        assert_eq!((e.year, e.week), (2025, 17));
    }

    #[test]
    fn rejects_bad_checksum() {
        let mut b = block([0x06, 0xB3], "X");
        b[127] ^= 1;
        assert!(parse(&b).is_none());
    }
}
