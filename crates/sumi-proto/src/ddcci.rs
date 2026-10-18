//! DDC/CI message framing (VESA DDC/CI 1.1, MCCS 2.2).
//!
//! The host writes to I2C slave 0x37 (8-bit 0x6E) and frames each request as
//! `0x51, 0x80 | len, payload…, checksum`; the checksum XORs the destination
//! address 0x6E with every byte. Replies arrive as `0x6E, 0x80 | len,
//! payload…, checksum`, checksummed against the virtual host address 0x50.

use thiserror::Error;

/// 7-bit I2C address of the DDC/CI command channel.
pub const DDC_ADDR: u16 = 0x37;
/// 7-bit I2C address of the EDID EEPROM.
pub const EDID_ADDR: u16 = 0x50;

const HOST_SRC: u8 = 0x51;
const DISPLAY_DST: u8 = 0x6E;
const HOST_VIRTUAL: u8 = 0x50;

pub const OP_GET_VCP: u8 = 0x01;
pub const OP_GET_VCP_REPLY: u8 = 0x02;
pub const OP_SET_VCP: u8 = 0x03;
pub const OP_CAPS_REQUEST: u8 = 0xF3;
pub const OP_CAPS_REPLY: u8 = 0xE3;

/// Longest reply we read: a capabilities fragment of up to 32 data bytes
/// plus address, length, opcode, 2-byte offset and checksum.
pub const MAX_REPLY: usize = 38;
/// A Get VCP Feature reply is always 11 bytes on the wire.
pub const VCP_REPLY_LEN: usize = 11;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("reply too short ({0} bytes)")]
    Short(usize),
    #[error("reply from unexpected source 0x{0:02x}")]
    Source(u8),
    #[error("bad length byte 0x{0:02x}")]
    Length(u8),
    #[error("checksum mismatch (got 0x{got:02x}, want 0x{want:02x})")]
    Checksum { got: u8, want: u8 },
    #[error("display sent a null message (busy or nothing to report)")]
    Null,
    #[error("unexpected reply opcode 0x{0:02x}")]
    Opcode(u8),
    #[error("reply is for VCP 0x{got:02x}, asked for 0x{want:02x}")]
    WrongCode { got: u8, want: u8 },
    #[error("display reports VCP 0x{0:02x} as unsupported")]
    Unsupported(u8),
    #[error("capabilities fragment for offset {got}, asked for {want}")]
    WrongOffset { got: u16, want: u16 },
}

fn xor(init: u8, bytes: &[u8]) -> u8 {
    bytes.iter().fold(init, |a, b| a ^ b)
}

/// Frame a host → display request around `payload` (opcode first).
pub fn request(payload: &[u8]) -> Vec<u8> {
    assert!(payload.len() <= 0x7F);
    let mut out = Vec::with_capacity(payload.len() + 3);
    out.push(HOST_SRC);
    out.push(0x80 | payload.len() as u8);
    out.extend_from_slice(payload);
    out.push(xor(DISPLAY_DST, &out));
    out
}

pub fn get_vcp_request(code: u8) -> Vec<u8> {
    request(&[OP_GET_VCP, code])
}

pub fn set_vcp_request(code: u8, value: u16) -> Vec<u8> {
    let [hi, lo] = value.to_be_bytes();
    request(&[OP_SET_VCP, code, hi, lo])
}

pub fn caps_request(offset: u16) -> Vec<u8> {
    let [hi, lo] = offset.to_be_bytes();
    request(&[OP_CAPS_REQUEST, hi, lo])
}

/// Validate a display → host frame and return its payload.
pub fn parse_reply(buf: &[u8]) -> Result<&[u8], FrameError> {
    if buf.len() < 3 {
        return Err(FrameError::Short(buf.len()));
    }
    if buf[0] != DISPLAY_DST {
        return Err(FrameError::Source(buf[0]));
    }
    if buf[1] & 0x80 == 0 {
        return Err(FrameError::Length(buf[1]));
    }
    let len = (buf[1] & 0x7F) as usize;
    if buf.len() < len + 3 {
        return Err(FrameError::Short(buf.len()));
    }
    let want = xor(HOST_VIRTUAL, &buf[..len + 2]);
    let got = buf[len + 2];
    if got != want {
        return Err(FrameError::Checksum { got, want });
    }
    if len == 0 {
        return Err(FrameError::Null);
    }
    Ok(&buf[2..len + 2])
}

/// VCP type byte from a Get VCP Feature reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VcpType {
    SetParameter,
    Momentary,
}

/// A decoded Get VCP Feature reply: the 16-bit maximum and current value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct VcpReply {
    pub code: u8,
    pub kind: VcpType,
    pub max: u16,
    pub current: u16,
}

pub fn parse_vcp_reply(buf: &[u8], want: u8) -> Result<VcpReply, FrameError> {
    let p = parse_reply(buf)?;
    if p[0] != OP_GET_VCP_REPLY {
        return Err(FrameError::Opcode(p[0]));
    }
    if p.len() < 8 {
        return Err(FrameError::Short(buf.len()));
    }
    if p[2] != want {
        return Err(FrameError::WrongCode { got: p[2], want });
    }
    if p[1] != 0 {
        return Err(FrameError::Unsupported(want));
    }
    Ok(VcpReply {
        code: want,
        kind: if p[3] == 1 { VcpType::Momentary } else { VcpType::SetParameter },
        max: u16::from_be_bytes([p[4], p[5]]),
        current: u16::from_be_bytes([p[6], p[7]]),
    })
}

/// Parse one capabilities fragment; an empty slice marks the end.
pub fn parse_caps_fragment(buf: &[u8], want: u16) -> Result<&[u8], FrameError> {
    let p = parse_reply(buf)?;
    if p[0] != OP_CAPS_REPLY {
        return Err(FrameError::Opcode(p[0]));
    }
    if p.len() < 3 {
        return Err(FrameError::Short(buf.len()));
    }
    let got = u16::from_be_bytes([p[1], p[2]]);
    if got != want {
        return Err(FrameError::WrongOffset { got, want });
    }
    Ok(&p[3..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_request_matches_spec_example() {
        // Get VCP 0x10 (brightness): 51 82 01 10 chk, chk = 6E^51^82^01^10.
        assert_eq!(get_vcp_request(0x10), vec![0x51, 0x82, 0x01, 0x10, 0xAC]);
    }

    #[test]
    fn set_request_frames_value_big_endian() {
        let f = set_vcp_request(0x10, 0x0032);
        assert_eq!(&f[..6], &[0x51, 0x84, 0x03, 0x10, 0x00, 0x32]);
        assert_eq!(f[6], 0x6E ^ 0x51 ^ 0x84 ^ 0x03 ^ 0x10 ^ 0x00 ^ 0x32);
    }

    fn reply(payload: &[u8]) -> Vec<u8> {
        let mut v = vec![0x6E, 0x80 | payload.len() as u8];
        v.extend_from_slice(payload);
        v.push(xor(0x50, &v));
        v
    }

    #[test]
    fn vcp_reply_decodes_max_and_current() {
        // PG32UCDM3 0xFD: max 0x7879, current 0x7829.
        let r = parse_vcp_reply(&reply(&[0x02, 0x00, 0xFD, 0x00, 0x78, 0x79, 0x78, 0x29]), 0xFD).unwrap();
        assert_eq!((r.max, r.current, r.kind), (0x7879, 0x7829, VcpType::SetParameter));
    }

    #[test]
    fn vcp_reply_errors() {
        let ok = reply(&[0x02, 0x00, 0x10, 0x00, 0x00, 0x64, 0x00, 0x32]);
        assert_eq!(parse_vcp_reply(&ok, 0x12), Err(FrameError::WrongCode { got: 0x10, want: 0x12 }));
        let unsup = reply(&[0x02, 0x01, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00]);
        assert_eq!(parse_vcp_reply(&unsup, 0x10), Err(FrameError::Unsupported(0x10)));
        let mut bad = ok.clone();
        bad[5] ^= 1;
        assert!(matches!(parse_vcp_reply(&bad, 0x10), Err(FrameError::Checksum { .. })));
        assert_eq!(parse_reply(&[0x6E, 0x80, 0xBE]), Err(FrameError::Null));
    }

    #[test]
    fn caps_fragment() {
        let f = reply(&[0xE3, 0x00, 0x20, b'(', b'p']);
        assert_eq!(parse_caps_fragment(&f, 0x20).unwrap(), b"(p");
        assert_eq!(parse_caps_fragment(&reply(&[0xE3, 0x01, 0x00]), 0x100).unwrap(), b"");
    }
}
