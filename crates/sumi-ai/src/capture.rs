//! Screen capture through KWin's ScreenShot2 D-Bus interface (Plasma 6).
//!
//! KWin only serves processes whose executable is listed in a desktop file
//! with `X-KDE-DBUS-Restricted-Interfaces=org.kde.KWin.ScreenShot2` (Sumi
//! installs one for `sumid`). Frames stay in memory and are never written out.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsFd, FromRawFd, OwnedFd};

use anyhow::{bail, Context, Result};
use zbus::blocking::Connection;
use zbus::zvariant::{Fd, OwnedValue, Value};

use crate::preprocess::{Frame, PixelFormat};

/// An owned captured frame.
pub struct Capture {
    pub data: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub format: PixelFormat,
}

impl Capture {
    pub fn frame(&self) -> Frame<'_> {
        Frame { data: &self.data, width: self.width, height: self.height, stride: self.stride, format: self.format }
    }
}

fn pipe() -> Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0; 2];
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

fn get_u32(results: &HashMap<String, OwnedValue>, key: &str) -> Result<u32> {
    let v = results.get(key).with_context(|| format!("KWin reply has no {key}"))?;
    Ok(u32::try_from(v.try_clone()?)?)
}

/// Capture one output (KWin output name, e.g. "DP-3") at its logical size.
pub fn capture_output(conn: &Connection, output: &str) -> Result<Capture> {
    let (read_end, write_end) = pipe()?;
    let mut options: HashMap<&str, Value> = HashMap::new();
    options.insert("include-cursor", Value::from(false));
    options.insert("native-resolution", Value::from(false));
    let reply = conn
        .call_method(
            Some("org.kde.KWin"),
            "/org/kde/KWin/ScreenShot2",
            Some("org.kde.KWin.ScreenShot2"),
            "CaptureScreen",
            &(output, options, Fd::from(write_end.as_fd())),
        )
        .context("KWin ScreenShot2 (is sumid's desktop file installed?)")?;
    // KWin writes on its own thread after replying; drop our copy of the
    // write end so the read sees EOF when KWin is done.
    drop(write_end);
    let results: HashMap<String, OwnedValue> = reply.body().deserialize()?;
    let kind = results.get("type").and_then(|v| String::try_from(v.try_clone().ok()?).ok()).unwrap_or_default();
    if kind != "raw" {
        bail!("KWin returned a {kind:?} screenshot, expected raw pixels");
    }
    let width = get_u32(&results, "width")? as usize;
    let height = get_u32(&results, "height")? as usize;
    let stride = get_u32(&results, "stride")? as usize;
    let qformat = get_u32(&results, "format")?;
    let format = PixelFormat::from_qimage(qformat).with_context(|| format!("unsupported QImage format {qformat}"))?;
    let mut data = Vec::with_capacity(stride * height);
    File::from(read_end).read_to_end(&mut data)?;
    if data.len() < stride * height || stride < width * 4 {
        bail!("short screenshot: {} bytes for {width}×{height} (stride {stride})", data.len());
    }
    Ok(Capture { data, width, height, stride, format })
}
