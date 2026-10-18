//! Map i2c buses to DRM connectors ("DP-3") by comparing EDIDs.
//!
//! The NVIDIA driver exposes neither a `ddc` symlink nor an `edid` file in
//! sysfs, so the connector's EDID is read through the DRM connector property
//! ioctls on `/dev/dri/cardN`. These need no DRM master (a non-master
//! GETCONNECTOR never forces a probe) and change nothing.

use std::fs::{self, File};
use std::os::fd::AsRawFd;

const DRM_IOCTL_MODE_GETCONNECTOR: libc::c_ulong = 0xC050_64A7;
const DRM_IOCTL_MODE_GETPROPERTY: libc::c_ulong = 0xC040_64AA;
const DRM_IOCTL_MODE_GETPROPBLOB: libc::c_ulong = 0xC010_64AC;

#[repr(C)]
#[derive(Default)]
struct GetConnector {
    encoders_ptr: u64,
    modes_ptr: u64,
    props_ptr: u64,
    prop_values_ptr: u64,
    count_modes: u32,
    count_props: u32,
    count_encoders: u32,
    encoder_id: u32,
    connector_id: u32,
    connector_type: u32,
    connector_type_id: u32,
    connection: u32,
    mm_width: u32,
    mm_height: u32,
    subpixel: u32,
    pad: u32,
}

#[repr(C)]
struct GetProperty {
    values_ptr: u64,
    enum_blob_ptr: u64,
    prop_id: u32,
    flags: u32,
    name: [u8; 32],
    count_values: u32,
    count_enum_blobs: u32,
}

#[repr(C)]
#[derive(Default)]
struct GetBlob {
    blob_id: u32,
    length: u32,
    data: u64,
}

fn ioctl<T>(f: &File, req: libc::c_ulong, arg: &mut T) -> bool {
    unsafe { libc::ioctl(f.as_raw_fd(), req, arg as *mut T) == 0 }
}

/// EDID blob of one connector, if it has one.
fn connector_edid(card: &File, connector_id: u32) -> Option<Vec<u8>> {
    let mut c = GetConnector { connector_id, ..Default::default() };
    if !ioctl(card, DRM_IOCTL_MODE_GETCONNECTOR, &mut c) || c.count_props == 0 {
        return None;
    }
    let n = c.count_props as usize;
    let mut props = vec![0u32; n];
    let mut values = vec![0u64; n];
    // Second call fills only the property arrays (modes/encoders stay at 0).
    let mut c = GetConnector {
        connector_id,
        count_props: n as u32,
        props_ptr: props.as_mut_ptr() as u64,
        prop_values_ptr: values.as_mut_ptr() as u64,
        ..Default::default()
    };
    if !ioctl(card, DRM_IOCTL_MODE_GETCONNECTOR, &mut c) {
        return None;
    }
    let n = (c.count_props as usize).min(n);
    for i in 0..n {
        let mut p = GetProperty { values_ptr: 0, enum_blob_ptr: 0, prop_id: props[i], flags: 0, name: [0; 32], count_values: 0, count_enum_blobs: 0 };
        if !ioctl(card, DRM_IOCTL_MODE_GETPROPERTY, &mut p) {
            continue;
        }
        let name_len = p.name.iter().position(|&b| b == 0).unwrap_or(32);
        if &p.name[..name_len] != b"EDID" || values[i] == 0 {
            continue;
        }
        let mut b = GetBlob { blob_id: values[i] as u32, ..Default::default() };
        if !ioctl(card, DRM_IOCTL_MODE_GETPROPBLOB, &mut b) || b.length == 0 {
            return None;
        }
        let mut data = vec![0u8; b.length as usize];
        let mut b = GetBlob { blob_id: values[i] as u32, length: b.length, data: data.as_mut_ptr() as u64 };
        if !ioctl(card, DRM_IOCTL_MODE_GETPROPBLOB, &mut b) {
            return None;
        }
        data.truncate(b.length as usize);
        return Some(data);
    }
    None
}

/// Connected DRM connectors as (name like "DP-3", EDID bytes).
pub fn connector_edids() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/drm") else { return out };
    for e in entries.flatten() {
        let full = e.file_name().to_string_lossy().into_owned();
        // "card1-DP-3" → card "card1", connector "DP-3".
        let Some((card, conn)) = full.split_once('-') else { continue };
        if !card.starts_with("card") || conn.starts_with("Writeback") {
            continue;
        }
        if fs::read_to_string(e.path().join("status")).map(|s| s.trim() != "connected").unwrap_or(true) {
            continue;
        }
        // Prefer the sysfs EDID when the driver provides one.
        let sysfs = fs::read(e.path().join("edid")).ok().filter(|b| !b.is_empty());
        let edid = sysfs.or_else(|| {
            let id: u32 = fs::read_to_string(e.path().join("connector_id")).ok()?.trim().parse().ok()?;
            let card = File::open(format!("/dev/dri/{card}")).ok()?;
            connector_edid(&card, id)
        });
        if let Some(edid) = edid {
            out.push((conn.to_string(), edid));
        }
    }
    out.sort();
    out
}

/// The connector whose EDID base block matches `block`.
pub fn connector_for(block: &[u8], connectors: &[(String, Vec<u8>)]) -> Option<String> {
    let base = &block[..block.len().min(128)];
    connectors.iter().find(|(_, e)| e.len() >= 128 && &e[..128] == base).map(|(n, _)| n.clone())
}

#[cfg(test)]
mod tests {
    #[test]
    fn ioctl_struct_sizes() {
        assert_eq!(std::mem::size_of::<super::GetConnector>(), 80);
        assert_eq!(std::mem::size_of::<super::GetProperty>(), 64);
        assert_eq!(std::mem::size_of::<super::GetBlob>(), 16);
    }
}
