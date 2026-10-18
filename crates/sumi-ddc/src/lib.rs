//! i2c-dev DDC/CI transport: monitor discovery by EDID and VCP Get/Set.
//!
//! Each transaction holds an exclusive `flock` on the `/dev/i2c-N` node, the
//! same lock ddcutil takes, so Sumi and ddcutil can share a bus safely.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

use sumi_proto::{caps, ddcci, edid, Capabilities, Edid, FrameError, VcpReply};
use thiserror::Error;

pub mod drm;

const I2C_SLAVE: libc::c_ulong = 0x0703;

#[derive(Debug, Error)]
pub enum DdcError {
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("VCP 0x{code:02x}: {source}")]
    Frame { code: u8, source: FrameError },
    #[error("capabilities: {0}")]
    Caps(FrameError),
    #[error("refusing to write VCP 0x{0:02x} without write access enabled")]
    WriteBlocked(u8),
    #[error("{0}: bus is locked by another process")]
    Locked(PathBuf),
    #[error("{0}: no EDID")]
    NoEdid(PathBuf),
}

pub type Result<T> = std::result::Result<T, DdcError>;

/// Delays from the DDC/CI spec; the ASUS app adds 20 ms on top of each call.
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    /// Request → reply read.
    pub reply: Duration,
    /// After a Set, before the next message.
    pub after_set: Duration,
    /// Minimum gap between any two messages.
    pub gap: Duration,
    pub retries: u32,
}

impl Timing {
    /// Scale every delay (like ddcutil's sleep multiplier); retries unchanged.
    pub fn scaled(f: f32) -> Self {
        let d = Timing::default();
        let s = |x: Duration| x.mul_f32(f.max(0.05));
        Timing { reply: s(d.reply), after_set: s(d.after_set), gap: s(d.gap), retries: d.retries }
    }
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            reply: Duration::from_millis(40),
            after_set: Duration::from_millis(50),
            gap: Duration::from_millis(50),
            retries: 4,
        }
    }
}

/// An open DDC/CI channel on one i2c bus.
pub struct Display {
    file: File,
    pub path: PathBuf,
    pub bus: u32,
    pub timing: Timing,
    writes: bool,
    ready_at: Instant,
}

/// A monitor found on an i2c bus.
#[derive(Debug, Clone)]
pub struct Found {
    pub bus: u32,
    pub path: PathBuf,
    pub adapter: String,
    pub edid: Edid,
    /// DRM connector name ("DP-3") whose EDID matches, if found.
    pub connector: Option<String>,
}

impl Found {
    pub fn is_asus(&self) -> bool {
        self.edid.manufacturer == "AUS"
    }
}

/// Holds the bus `flock` until dropped (the fd outlives it: it is owned by `Display`).
struct Lock(libc::c_int);

impl Drop for Lock {
    fn drop(&mut self) {
        unsafe { libc::flock(self.0, libc::LOCK_UN) };
    }
}

impl Display {
    pub fn open(bus: u32) -> Result<Self> {
        let path = PathBuf::from(format!("/dev/i2c-{bus}"));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|source| DdcError::Io { path: path.clone(), source })?;
        Ok(Display { file, path, bus, timing: Timing::default(), writes: false, ready_at: Instant::now() })
    }

    /// Allow `set_vcp`. Off by default so read-only tools cannot write by accident.
    pub fn allow_writes(&mut self, yes: bool) {
        self.writes = yes;
    }

    pub fn writes_allowed(&self) -> bool {
        self.writes
    }

    fn io(&self, source: std::io::Error) -> DdcError {
        DdcError::Io { path: self.path.clone(), source }
    }

    fn lock(&self) -> Result<Lock> {
        let fd = self.file.as_raw_fd();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                return Ok(Lock(fd));
            }
            if Instant::now() >= deadline {
                return Err(DdcError::Locked(self.path.clone()));
            }
            sleep(Duration::from_millis(20));
        }
    }

    fn address(&self, addr: u16) -> Result<()> {
        if unsafe { libc::ioctl(self.file.as_raw_fd(), I2C_SLAVE, addr as libc::c_ulong) } < 0 {
            return Err(self.io(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    fn wait_ready(&self) {
        let now = Instant::now();
        if self.ready_at > now {
            sleep(self.ready_at - now);
        }
    }

    fn write_frame(&mut self, frame: &[u8]) -> Result<()> {
        self.wait_ready();
        let r = (&self.file).write_all(frame);
        self.ready_at = Instant::now() + self.timing.gap;
        r.map_err(|e| self.io(e))
    }

    fn read_frame(&mut self, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        let n = (&self.file).read(&mut buf).map_err(|e| self.io(e))?;
        buf.truncate(n);
        Ok(buf)
    }

    /// Request/reply exchange with retries on transport and framing errors.
    fn exchange<T>(
        &mut self,
        frame: &[u8],
        reply_len: usize,
        mut decode: impl FnMut(&[u8]) -> std::result::Result<T, FrameError>,
        wrap: impl Fn(FrameError) -> DdcError,
    ) -> Result<T> {
        let _lock = self.lock()?;
        self.address(ddcci::DDC_ADDR)?;
        let mut delay = self.timing.reply;
        let mut last = None;
        for _ in 0..=self.timing.retries {
            if let Err(e) = self.write_frame(frame) {
                last = Some(e);
                sleep(delay);
                continue;
            }
            sleep(delay);
            match self.read_frame(reply_len) {
                Ok(buf) => match decode(&buf) {
                    Ok(v) => return Ok(v),
                    // A clean "unsupported" answer is final, not worth retrying.
                    Err(e @ FrameError::Unsupported(_)) => return Err(wrap(e)),
                    Err(e) => last = Some(wrap(e)),
                },
                Err(e) => last = Some(e),
            }
            delay = delay * 3 / 2;
        }
        Err(last.expect("at least one attempt"))
    }

    pub fn get_vcp(&mut self, code: u8) -> Result<VcpReply> {
        let frame = ddcci::get_vcp_request(code);
        self.exchange(
            &frame,
            ddcci::VCP_REPLY_LEN,
            |b| ddcci::parse_vcp_reply(b, code),
            |source| DdcError::Frame { code, source },
        )
    }

    /// Fire-and-forget Set VCP Feature. Callers verify by reading back.
    pub fn set_vcp(&mut self, code: u8, value: u16) -> Result<()> {
        if !self.writes {
            return Err(DdcError::WriteBlocked(code));
        }
        let _lock = self.lock()?;
        self.address(ddcci::DDC_ADDR)?;
        self.write_frame(&ddcci::set_vcp_request(code, value))?;
        self.ready_at = Instant::now() + self.timing.after_set.max(self.timing.gap);
        Ok(())
    }

    /// Read the full capability string, fragment by fragment.
    pub fn capabilities_raw(&mut self) -> Result<String> {
        let mut out = Vec::new();
        loop {
            let offset = out.len() as u16;
            let frame = ddcci::caps_request(offset);
            let chunk = self.exchange(
                &frame,
                ddcci::MAX_REPLY,
                |b| ddcci::parse_caps_fragment(b, offset).map(<[u8]>::to_vec),
                DdcError::Caps,
            )?;
            if chunk.is_empty() || out.len() > 8192 {
                break;
            }
            out.extend_from_slice(&chunk);
        }
        while out.last() == Some(&0) {
            out.pop();
        }
        Ok(String::from_utf8_lossy(&out).into_owned())
    }

    pub fn capabilities(&mut self) -> Result<Capabilities> {
        self.capabilities_raw().map(|s| caps::parse(&s))
    }

    /// Read the 128-byte EDID base block from the EEPROM at 0x50.
    pub fn read_edid(&mut self) -> Result<Vec<u8>> {
        let _lock = self.lock()?;
        self.address(ddcci::EDID_ADDR)?;
        (&self.file).write_all(&[0]).map_err(|e| self.io(e))?;
        let mut buf = vec![0u8; edid::BLOCK_LEN];
        (&self.file).read_exact(&mut buf).map_err(|e| self.io(e))?;
        Ok(buf)
    }
}

/// i2c adapters that can carry a monitor's DDC lines (GPU drivers only:
/// skip SMBus and other board-level controllers).
fn is_display_adapter(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["nvidia", "amdgpu", "radeon", "i915", "dp-aux", "dp aux", "nouveau", "dpmst"].iter().any(|k| n.contains(k))
        && !n.contains("smbus")
}

/// Scan GPU i2c buses for monitors with a readable EDID.
pub fn discover() -> Vec<Found> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/bus/i2c/devices") else { return out };
    let mut buses: Vec<(u32, String)> = entries
        .flatten()
        .filter_map(|e| {
            let n: u32 = e.file_name().to_str()?.strip_prefix("i2c-")?.parse().ok()?;
            let name = fs::read_to_string(e.path().join("name")).ok()?.trim().to_string();
            is_display_adapter(&name).then_some((n, name))
        })
        .collect();
    buses.sort();
    let connectors = drm::connector_edids();
    for (bus, adapter) in buses {
        let Ok(mut d) = Display::open(bus) else { continue };
        let Ok(block) = d.read_edid() else { continue };
        if let Some(edid) = edid::parse(&block) {
            let connector = drm::connector_for(&block, &connectors);
            out.push(Found { bus, path: d.path.clone(), adapter, edid, connector });
        }
    }
    out
}
