//! Capture one KWin output and save it as a PNG (test aid for the virtual-KWin GUI checks).
//!
//!     capture-png <output> <file.png> [x y w h]
//!
//! Needs the same desktop-file authorisation as capture-probe (see tests/kwin-capture.sh).
//! The optional rectangle crops the saved image (logical pixels).

use sumi_ai::preprocess::PixelFormat;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let output = args.next().unwrap_or_else(|| "Virtual-0".into());
    let path = args.next().unwrap_or_else(|| "capture.png".into());
    let rect: Vec<usize> = args.filter_map(|a| a.parse().ok()).collect();
    let conn = zbus::blocking::Connection::session()?;
    let cap = sumi_ai::capture::capture_output(&conn, &output)?;
    let f = cap.frame();
    let (cx, cy, cw, ch) = match rect[..] {
        [x, y, w, h] => (x.min(f.width), y.min(f.height), w.min(f.width.saturating_sub(x)), h.min(f.height.saturating_sub(y))),
        _ => (0, 0, f.width, f.height),
    };
    let mut rgba = Vec::with_capacity(cw * ch * 4);
    for y in cy..cy + ch {
        for x in cx..cx + cw {
            let i = y * f.stride + x * 4;
            let p = &f.data[i..i + 4];
            match f.format {
                PixelFormat::Bgra => rgba.extend_from_slice(&[p[2], p[1], p[0], 255]),
                PixelFormat::Rgba => rgba.extend_from_slice(&[p[0], p[1], p[2], 255]),
            }
        }
    }
    image::RgbaImage::from_raw(cw as u32, ch as u32, rgba).ok_or_else(|| anyhow::anyhow!("bad image size"))?.save(&path)?;
    println!("{output}: captured {}x{}, saved {cw}x{ch} to {path}", f.width, f.height);
    Ok(())
}
