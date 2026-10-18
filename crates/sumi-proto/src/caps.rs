//! MCCS capability string parser.
//!
//! The string is a parenthesised tree, e.g.
//! `(prot(monitor)type(LCD)model(PG32UCDM3)vcp(02 10 14(05 08) …)mccs_ver(2.2))`.
//! `vcp(…)` lists feature codes, each optionally followed by the values it
//! accepts. ASUS mixes 2- and 4-digit hex tokens in one list
//! (`DC(0000 0100 01 02 …)`), so each value keeps its digit count.

use std::collections::BTreeMap;

use serde::Serialize;

/// One value token from a `vcp(code(values…))` list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CapValue {
    pub value: u16,
    /// Hex digits as written (2 or 4).
    pub digits: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Capabilities {
    /// Top-level entries other than `vcp` and `cmds`, raw (`model`, `type`, `mccs_ver`…).
    pub fields: BTreeMap<String, String>,
    pub cmds: Vec<u8>,
    /// Feature code → advertised values (empty when none are listed).
    pub vcp: BTreeMap<u8, Vec<CapValue>>,
}

impl Capabilities {
    pub fn model(&self) -> Option<&str> {
        self.fields.get("model").map(String::as_str)
    }

    pub fn mccs_version(&self) -> Option<&str> {
        self.fields.get("mccs_ver").map(String::as_str)
    }

    pub fn supports(&self, code: u8) -> bool {
        self.vcp.contains_key(&code)
    }

    pub fn values(&self, code: u8) -> &[CapValue] {
        self.vcp.get(&code).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Whether `code` lists `value` written with `digits` hex digits.
    pub fn has_value(&self, code: u8, value: u16, digits: u8) -> bool {
        self.values(code).iter().any(|v| v.value == value && v.digits == digits)
    }
}

/// Split `s` into top-level `name(body)` pairs; `body` keeps nested parens.
fn entries(s: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let start = i;
        while i < b.len() && b[i] != b'(' && b[i] != b')' {
            i += 1;
        }
        if i >= b.len() || b[i] == b')' {
            break;
        }
        let name = s[start..i].trim();
        let open = i;
        let mut depth = 0usize;
        while i < b.len() {
            match b[i] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        let body = &s[open + 1..i.min(b.len())];
        out.push((name, body));
        i += 1;
    }
    out
}

fn hex_tokens(s: &str) -> impl Iterator<Item = (u16, u8)> + '_ {
    s.split_ascii_whitespace().filter_map(|t| {
        let v = u16::from_str_radix(t, 16).ok()?;
        Some((v, t.len().min(255) as u8))
    })
}

fn parse_vcp(body: &str) -> BTreeMap<u8, Vec<CapValue>> {
    let mut out = BTreeMap::new();
    let b = body.as_bytes();
    let mut i = 0;
    let mut last: Option<u8> = None;
    while i < b.len() {
        match b[i] {
            b'(' => {
                let open = i;
                let mut depth = 0usize;
                while i < b.len() {
                    match b[i] {
                        b'(' => depth += 1,
                        b')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                if let Some(code) = last {
                    let vals = hex_tokens(&body[open + 1..i.min(b.len())])
                        .map(|(value, digits)| CapValue { value, digits })
                        .collect();
                    out.insert(code, vals);
                }
                i += 1;
            }
            c if c.is_ascii_hexdigit() => {
                let start = i;
                while i < b.len() && b[i].is_ascii_hexdigit() {
                    i += 1;
                }
                if let Ok(code) = u8::from_str_radix(&body[start..i], 16) {
                    out.entry(code).or_insert_with(Vec::new);
                    last = Some(code);
                }
            }
            _ => i += 1,
        }
    }
    out
}

pub fn parse(raw: &str) -> Capabilities {
    let s = raw.trim_matches(|c: char| c.is_whitespace() || c == '\0');
    // The whole string is normally wrapped in one outer pair of parens.
    let inner = match entries(s).as_slice() {
        [("", body)] => *body,
        _ => s,
    };
    let mut caps = Capabilities::default();
    for (name, body) in entries(inner) {
        match name.to_ascii_lowercase().as_str() {
            "vcp" => caps.vcp = parse_vcp(body),
            "cmds" => caps.cmds = hex_tokens(body).map(|(v, _)| v as u8).collect(),
            other => {
                caps.fields.insert(other.to_string(), body.trim().to_string());
            }
        }
    }
    caps
}

#[cfg(test)]
mod tests {
    use super::*;

    const PG32UCDM3: &str = "(prot(monitor)type(LCD)model(PG32UCDM3)cmds(01 02 03 07 0C E3 F3)vcp(02 04 05 08 10 12 14(03 04 05 06 07 08 09 0B) 16 18 1A 52 59 5A 5B 5C 5D 5E 5F(0005) 60(0F 11 12 1A) 61(01 02 03) 62 72(5000 6400 7800 8C00 A000) 87 8A 8D(01 02) AC AE B2 B6 C0 C6 C8 CC(01 02 03 04 05 06 07 08 09 0A 0C 0D 11 12 14 1A 1E 1F 23 24 25 26 27) D6(01 05) DC(0000 0100 0200 01 02 04 05 06 07 08 09 0A) DF E1(00 01) E2(0000 0100 0200 01 02 03 05 06 07 08) E3(00 07 08 09 0A 0B 0C 0D 0E 0F) E4(00 01 02 03 04 05) E5(00 01 02 03 04) E6(00 01 02 03 04) E7(00 01) E8(01 02 03 04) E9 EA(00 01 02) EB(00 01 02 03 04 05 06 07) EC(01 FF) ED(0000 0500 0A00 0F00 00 01 02 03 FF) EE(00 01) EF F2(0000 0100 0200 0300 0400 0500 0600 00 01 02 03 04 05 06) F3(02 03 04) F4(00 01 02 03 04 05) F5(0F00 1100 1200 1A00 0F 11 12 1A) F6(0100 0200 0400 0500 0600 0700 0800 0900 0A00 01 02 04 05 06 07 08 09 0A ) F7(0000 0100 0200 0300 0400 00 01 02 03) F8(00 02 04 08) F9(00 01 02 03) FA(01 02 03 04) FB(0000 0100 0200 0300 0400 0500 0600 0700 0800 0900 0A00 0B00 0C00 00 01 02 03 04 05 06 07 08 09 0A 0B 0C) FC(089F) FD(7879) FF)mswhql(1)mccs_ver(2.2)asset_eep(40)\t";

    #[test]
    fn parses_pg32ucdm3() {
        let c = parse(PG32UCDM3);
        assert_eq!(c.model(), Some("PG32UCDM3"));
        assert_eq!(c.mccs_version(), Some("2.2"));
        assert_eq!(c.cmds, vec![0x01, 0x02, 0x03, 0x07, 0x0C, 0xE3, 0xF3]);
        assert!(c.supports(0x10) && c.values(0x10).is_empty());
        assert_eq!(c.values(0x60).iter().map(|v| v.value).collect::<Vec<_>>(), vec![0x0F, 0x11, 0x12, 0x1A]);
        assert!(c.has_value(0xDC, 0x0100, 4) && c.has_value(0xDC, 0x01, 2));
        assert!(!c.has_value(0xDC, 0x03, 2));
        assert_eq!(c.values(0xFD), &[CapValue { value: 0x7879, digits: 4 }]);
        assert!(c.supports(0xFF) && !c.supports(0xDD));
        assert_eq!(c.vcp.len(), 64);
    }

    #[test]
    fn tolerates_lower_case_and_missing_outer_parens() {
        let c = parse("model(X)vcp(10 e3(07 0a 0b))");
        assert_eq!(c.model(), Some("X"));
        assert_eq!(c.values(0xE3).len(), 3);
    }
}
