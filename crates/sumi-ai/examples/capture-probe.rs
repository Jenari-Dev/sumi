//! Capture one KWin output and print its size and average colour (test aid).
//!
//!     cargo run -p sumi-ai --example capture-probe -- DP-3

fn main() -> anyhow::Result<()> {
    let output = std::env::args().nth(1).unwrap_or_else(|| "Virtual-0".into());
    let conn = zbus::blocking::Connection::session()?;
    let t = std::time::Instant::now();
    let cap = sumi_ai::capture::capture_output(&conn, &output)?;
    let took = t.elapsed();
    let f = cap.frame();
    let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
    for y in (0..f.height).step_by(8) {
        for x in (0..f.width).step_by(8) {
            let i = y * f.stride + x * 4;
            let p = &f.data[i..i + 4];
            let (pr, pg, pb) = match f.format {
                sumi_ai::preprocess::PixelFormat::Bgra => (p[2], p[1], p[0]),
                sumi_ai::preprocess::PixelFormat::Rgba => (p[0], p[1], p[2]),
            };
            r += pr as u64;
            g += pg as u64;
            b += pb as u64;
            n += 1;
        }
    }
    let t2 = std::time::Instant::now();
    let tensor = sumi_ai::preprocess::to_tensor(&f);
    println!(
        "{output}: {}x{} stride {} {:?} in {took:?}; mean rgb {} {} {}; tensor {} values in {:?}",
        f.width, f.height, f.stride, f.format, r / n, g / n, b / n, tensor.len(), t2.elapsed()
    );
    Ok(())
}
