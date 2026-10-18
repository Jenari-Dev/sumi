//! `sumi` — command-line control for ASUS monitors over DDC/CI.

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use sumi_ddc::{discover, Display};

#[derive(Parser)]
#[command(version, about = "Control ASUS monitors over DDC/CI")]
struct Cli {
    /// i2c bus number (default: the first ASUS monitor found).
    #[arg(long, global = true)]
    bus: Option<u32>,
    /// Print JSON instead of text.
    #[arg(long, global = true)]
    json: bool,
    /// Scale DDC/CI delays (1.0 = spec timing).
    #[arg(long, global = true, default_value_t = 1.0)]
    sleep_multiplier: f32,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// List monitors with a readable EDID on GPU i2c buses.
    Detect,
    /// Dump each connector's EDID as the kernel has it (no i2c traffic).
    Edid,
    /// Show the monitor's MCCS capability string.
    Caps {
        /// Print the raw string.
        #[arg(long)]
        raw: bool,
    },
    /// Read VCP features (hex codes, e.g. `get 10 dc fd`).
    Get {
        #[arg(required = true, value_parser = parse_hex8)]
        codes: Vec<u8>,
    },
    /// Read every VCP feature the capability string lists.
    Dump,
    /// Guided read-only probe: change one setting on the monitor's OSD, type
    /// what you changed, and see which VCP values moved. Logs JSON lines.
    Probe {
        /// JSON-lines log file (appended).
        #[arg(long, default_value = "sumi-probe.jsonl")]
        log: std::path::PathBuf,
        /// Only read these codes (hex); default: every readable advertised code.
        #[arg(long, value_delimiter = ',', value_parser = parse_hex8)]
        codes: Vec<u8>,
    },
    /// Decoded state of every supported feature.
    State,
    /// Change a feature by id (see `state`), verified by read-back.
    Apply {
        id: String,
        /// Value in the feature's units (option value, 0/1 for toggles).
        #[arg(value_parser = parse_u16)]
        value: u16,
        /// Required: actually send the write.
        #[arg(long)]
        confirm: bool,
    },
    /// End-to-end write test on harmless settings: change, verify, restore.
    WriteSelftest {
        /// Required: actually send the writes.
        #[arg(long)]
        confirm: bool,
        /// Also test every other harmless write (OLED toggles, GamePlus
        /// overlays, OSD settings, RGB gains, Blue Light Filter…).
        #[arg(long)]
        extended: bool,
    },
    /// Print every display string the daemon supplies (for translation catalogues).
    #[command(hide = true)]
    Strings,
    /// AI Visual model: status, download, classify an image file.
    Ai {
        #[command(subcommand)]
        cmd: AiCmd,
    },
    /// Firmware updates: ask ASUS's download server (only when you run this), download the zip.
    Firmware {
        #[command(subcommand)]
        cmd: FwCmd,
    },
    /// Write one VCP feature and read it back.
    Set {
        #[arg(value_parser = parse_hex8)]
        code: u8,
        /// Value (decimal, or hex with 0x).
        #[arg(value_parser = parse_u16)]
        value: u16,
        /// Required: actually send the write.
        #[arg(long)]
        confirm: bool,
    },
}

#[derive(Subcommand)]
enum FwCmd {
    /// HEAD-request the ten firmware versions after the current one (DisplayWidget's check).
    Check,
    /// Check, then save the newer firmware zip into your Download folder (never overwrites;
    /// Sumi doesn't flash anything: the zip holds ASUS's Windows update tool).
    Download,
}

#[derive(Subcommand)]
enum AiCmd {
    /// Is the model installed, and does this build carry prompt embeddings?
    Status,
    /// Download the vision model (asks first; shows file, source and size).
    Download {
        /// Don't ask.
        #[arg(long)]
        yes: bool,
    },
    /// Classify image files (PNG/JPEG) and print the scene scores.
    Classify { files: Vec<std::path::PathBuf> },
}

fn ai_cmd(cmd: AiCmd) -> Result<()> {
    use sumi_ai::{clip, model, preprocess};
    match cmd {
        AiCmd::Status => {
            println!("model:   {} ({} MB) {}", model::VISION.name, model::VISION.size / 1_000_000, if model::is_installed() { "installed" } else { "not installed" });
            println!("path:    {}", model::vision_path().display());
            println!("prompts: {}", match clip::shipped_prompts() { Ok(p) => format!("{} embedded ({})", p.prompts.len(), p.model), Err(e) => e.to_string() });
        }
        AiCmd::Download { yes } => {
            let f = model::VISION;
            println!("{}\n  from {}\n  {} bytes ({} MB), SHA-256 {}\n  to   {}", f.name, f.url, f.size, f.size / 1_000_000, f.sha256, model::model_dir().display());
            if !yes {
                print!("Download? [y/N] ");
                std::io::Write::flush(&mut std::io::stdout())?;
                let mut a = String::new();
                std::io::stdin().read_line(&mut a)?;
                if !a.trim().eq_ignore_ascii_case("y") {
                    bail!("not downloaded");
                }
            }
            let mut last = 0;
            let p = model::download(&f, |done, total| {
                let pct = done * 100 / total.max(1);
                if pct != last {
                    last = pct;
                    eprint!("\r{pct:>3} %");
                }
            })?;
            eprintln!();
            println!("installed {}", p.display());
        }
        AiCmd::Classify { files } => {
            let c = clip::Clip::load(&model::vision_path(), &clip::shipped_prompts()?)?;
            for path in files {
                let img = image::open(&path).with_context(|| format!("opening {}", path.display()))?.to_rgba8();
                let (w, h) = img.dimensions();
                let frame = preprocess::Frame { data: img.as_raw(), width: w as usize, height: h as usize, stride: w as usize * 4, format: preprocess::PixelFormat::Rgba };
                let t = std::time::Instant::now();
                let o = c.classify(&frame)?;
                let mut s = o.scores.clone();
                s.sort_by(|a, b| b.1.total_cmp(&a.1));
                let top: Vec<String> = s.iter().take(3).map(|(k, v)| format!("{k:?} {v:.0}%")).collect();
                println!("{:<40} game {:>3.0}%  conf {:>3.0}%  {}  ({:?})", path.file_name().unwrap_or_default().to_string_lossy(), o.is_game * 100.0, o.confidence * 100.0, top.join(", "), t.elapsed());
            }
        }
    }
    Ok(())
}

fn parse_hex8(s: &str) -> Result<u8, String> {
    u8::from_str_radix(s.trim_start_matches("0x").trim_start_matches("0X"), 16).map_err(|e| e.to_string())
}

fn parse_u16(s: &str) -> Result<u16, String> {
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(h) => u16::from_str_radix(h, 16),
        None => s.parse(),
    }
    .map_err(|e| e.to_string())
}

fn open(bus: Option<u32>, mult: f32) -> Result<Display> {
    let bus = match bus {
        Some(b) => b,
        None => discover().into_iter().find(|f| f.is_asus()).map(|f| f.bus).context("no ASUS monitor found")?,
    };
    let mut d = Display::open(bus)?;
    d.timing = sumi_ddc::Timing::scaled(mult);
    Ok(d)
}

fn attach(bus: Option<u32>) -> Result<sumi_core::Monitor> {
    let found = discover()
        .into_iter()
        .find(|f| bus.map_or(f.is_asus(), |b| f.bus == b))
        .context("no ASUS monitor found")?;
    sumi_core::Monitor::attach(found)
}

fn print_reply(r: &sumi_proto::VcpReply, json: bool) {
    if json {
        println!("{}", serde_json::to_string(r).unwrap());
    } else {
        println!("0x{:02X}  current {:>5} (0x{:04X})  max {:>5} (0x{:04X})", r.code, r.current, r.current, r.max, r.max);
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Ai { cmd } => ai_cmd(cmd)?,
        Cmd::Firmware { cmd } => {
            use sumi_core::firmware;
            let m = attach(cli.bus)?;
            let (model, fw) = (m.model().to_string(), m.fw.fw_version);
            let r = firmware::check(&model, fw, |url| {
                let s = firmware::http_head(url)?;
                eprintln!("HEAD {url} -> {s}");
                Ok(s)
            })?;
            if cli.json && matches!(cmd, FwCmd::Check) {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("{model}: current {}", r.current);
                match (&r.latest, &r.url) {
                    (Some(l), Some(u)) => println!("{l} is available\n  {u}"),
                    _ => println!("Up to date"),
                }
            }
            if let (FwCmd::Download, Some(url)) = (cmd, &r.url) {
                let mut last = 0;
                let p = firmware::download(url, &firmware::download_dir(), |done, total| {
                    let pct = done * 100 / total.max(1);
                    if pct != last {
                        last = pct;
                        eprint!("\r{pct:>3} %");
                    }
                })?;
                eprintln!();
                println!("Saved to {}\nRun ASUS's update tool from this file on Windows; Sumi doesn't flash firmware.", p.display());
            }
        }
        Cmd::Strings => {
            for s in sumi_core::strings::all() {
                println!("{s}");
            }
        }
        Cmd::Detect => {
            let found = discover();
            if cli.json {
                let v: Vec<_> = found
                    .iter()
                    .map(|f| serde_json::json!({ "bus": f.bus, "adapter": f.adapter, "connector": f.connector, "edid": f.edid }))
                    .collect();
                println!("{}", serde_json::to_string_pretty(&v)?);
            }
            for f in &found {
                if !cli.json {
                    println!(
                        "i2c-{:<3} {:<6} {} {:<16} product 0x{:04X}  {}",
                        f.bus,
                        f.connector.as_deref().unwrap_or("?"),
                        f.edid.manufacturer,
                        f.edid.name.as_deref().unwrap_or("?"),
                        f.edid.product_code,
                        f.adapter
                    );
                }
            }
        }
        Cmd::Edid => {
            for (conn, e) in sumi_ddc::drm::connector_edids() {
                // Dolby Vision vendor-specific video block: OUI 00-D0-46, stored LSB first.
                let dv = e.windows(3).any(|w| w == [0x46, 0xD0, 0x00]);
                let name = sumi_proto::edid::parse(&e).and_then(|x| x.name).unwrap_or_default();
                println!("{conn:<6} {name:<16} {} bytes  dolby_vision_block={dv}", e.len());
                if cli.json {
                    let hex: Vec<String> = e.iter().map(|b| format!("{b:02x}")).collect();
                    println!("{}", hex.join(""));
                }
            }
        }
        Cmd::Caps { raw } => {
            let mut d = open(cli.bus, cli.sleep_multiplier)?;
            let s = d.capabilities_raw()?;
            if raw {
                println!("{s}");
            } else {
                let c = sumi_proto::caps::parse(&s);
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&c)?);
                } else {
                    println!("model {}  mccs {}", c.model().unwrap_or("?"), c.mccs_version().unwrap_or("?"));
                    for (code, vals) in &c.vcp {
                        let v: Vec<String> = vals.iter().map(|v| format!("{:0w$X}", v.value, w = v.digits as usize)).collect();
                        println!("  0x{code:02X} {}", v.join(" "));
                    }
                }
            }
        }
        Cmd::Get { codes } => {
            let mut d = open(cli.bus, cli.sleep_multiplier)?;
            for code in codes {
                match d.get_vcp(code) {
                    Ok(r) => print_reply(&r, cli.json),
                    Err(e) => eprintln!("0x{code:02X}  error: {e}"),
                }
            }
        }
        Cmd::Dump => {
            let mut d = open(cli.bus, cli.sleep_multiplier)?;
            let c = d.capabilities()?;
            for &code in c.vcp.keys() {
                match d.get_vcp(code) {
                    Ok(r) => print_reply(&r, cli.json),
                    Err(e) => eprintln!("0x{code:02X}  error: {e}"),
                }
            }
        }
        Cmd::State => {
            let mut m = attach(cli.bus)?;
            let st = m.state(None);
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&st)?);
            } else {
                println!("{}  FW {}  {:?}  oled={}  {:?}", st.model, st.firmware.fw_version, st.line, st.oled, st.context);
                for f in &st.features {
                    let v = match (f.value, f.options.iter().find(|o| Some(o.value) == f.value)) {
                        (_, Some(o)) => o.label.to_string(),
                        (Some(v), None) => match f.max { Some(m) => format!("{v}/{m}"), None => v.to_string() },
                        (None, _) => "-".into(),
                    };
                    println!("  {:<24} {:<28} {:<22}{}", f.id, f.label, v, if f.enabled { "" } else { " (disabled)" });
                }
            }
        }
        Cmd::Apply { id, value, confirm } => {
            if !confirm {
                bail!("writes need --confirm");
            }
            let mut m = attach(cli.bus)?;
            m.display.allow_writes(true);
            println!("{}", m.apply(&id, value)?);
        }
        Cmd::WriteSelftest { confirm, extended } => {
            if !confirm {
                bail!("writes need --confirm");
            }
            let mut m = attach(cli.bus)?;
            m.display.allow_writes(true);
            write_selftest(&mut m, extended)?;
        }
        Cmd::Probe { log, codes } => probe(open(cli.bus, cli.sleep_multiplier.min(0.5))?, &log, codes)?,
        Cmd::Set { code, value, confirm } => {
            if !confirm {
                bail!("writes need --confirm");
            }
            let mut d = open(cli.bus, cli.sleep_multiplier)?;
            let before = d.get_vcp(code)?;
            d.allow_writes(true);
            d.set_vcp(code, value)?;
            let after = d.get_vcp(code)?;
            println!("0x{code:02X}: {} -> {} (wrote {value})", before.current, after.current);
            if after.current != value {
                bail!("read-back mismatch: wrote {value}, monitor reports {}", after.current);
            }
        }
    }
    Ok(())
}

type Snapshot = std::collections::BTreeMap<u8, Option<(u16, u16)>>;

fn snapshot(d: &mut Display, codes: &[u8]) -> Snapshot {
    codes.iter().map(|&c| (c, d.get_vcp(c).ok().map(|r| (r.max, r.current)))).collect()
}

fn fmt_val(v: Option<(u16, u16)>) -> String {
    match v {
        Some((max, cur)) => format!("{cur} (0x{cur:04X}) max 0x{max:04X}"),
        None => "unreadable".into(),
    }
}

fn probe(mut d: Display, log: &std::path::Path, mut codes: Vec<u8>) -> Result<()> {
    use std::io::{BufRead, Write};
    use sumi_proto::vcp;
    if codes.is_empty() {
        let caps = d.capabilities()?;
        codes = caps.vcp.keys().copied().filter(|c| !vcp::WRITE_ONLY.contains(c)).collect();
        codes.extend_from_slice(vcp::UNADVERTISED_READABLE);
        codes.sort();
        codes.dedup();
    }
    let mut out = std::fs::OpenOptions::new().create(true).append(true).open(log)?;
    let now = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|t| t.as_secs()).unwrap_or(0);
    let record = |out: &mut std::fs::File, label: &str, snap: &Snapshot, changes: &[(u8, Option<(u16, u16)>, Option<(u16, u16)>)]| -> Result<()> {
        let j = serde_json::json!({
            "t": now(),
            "label": label,
            "changes": changes.iter().map(|(c, a, b)| serde_json::json!({"code": format!("{c:02X}"), "before": a, "after": b})).collect::<Vec<_>>(),
            "snapshot": snap.iter().map(|(c, v)| (format!("{c:02X}"), serde_json::json!(v))).collect::<serde_json::Map<_, _>>(),
        });
        writeln!(out, "{j}")?;
        Ok(())
    };
    println!("Reading {} codes for the starting point (a few seconds)…", codes.len());
    let mut prev = snapshot(&mut d, &codes);
    record(&mut out, "baseline", &prev, &[])?;
    let stdin = std::io::stdin();
    loop {
        print!("\nChange ONE setting on the monitor's OSD, close the OSD, then type what you changed (or q to quit):\n> ");
        std::io::stdout().flush()?;
        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            break;
        }
        let label = line.trim();
        if label == "q" || label == "quit" {
            break;
        }
        println!("Reading…");
        let snap = snapshot(&mut d, &codes);
        let changes: Vec<_> = codes.iter().filter_map(|c| (prev[c] != snap[c]).then(|| (*c, prev[c], snap[c]))).collect();
        if changes.is_empty() {
            println!("  no VCP value changed");
        }
        for (c, a, b) in &changes {
            println!("  0x{c:02X} {:<34} {}  ->  {}", vcp::name(*c), fmt_val(*a), fmt_val(*b));
        }
        record(&mut out, if label.is_empty() { "(unlabelled)" } else { label }, &snap, &changes)?;
        prev = snap;
    }
    println!("Log: {}", log.display());
    Ok(())
}

/// Harmless round trips: (feature, temporary value chooser). Each feature is
/// read, set to a different allowed value, verified, then restored and verified.
fn write_selftest(m: &mut sumi_core::Monitor, extended: bool) -> Result<()> {
    let st = m.state(None);
    let pick = |id: &str| -> Option<(u16, u16)> {
        let f = st.features.iter().find(|f| f.id == id && f.supported)?;
        let cur = f.value?;
        let other = if !f.options.is_empty() {
            f.options.iter().map(|o| o.value).find(|&v| v != cur)?
        } else if let Some(max) = f.max {
            if cur > 0 { cur - 1 } else { (cur + 1).min(max) }
        } else {
            u16::from(cur == 0)
        };
        Some((cur, other))
    };
    let mut tests = vec!["brightness", "contrast", "shadow_boost", "color_temp", "pixel_cleaning_reminder", "screen_move", "logo_detection", "gamevisual"];
    if extended {
        tests.extend([
            "saturation", "sharpness", "gamma", "volume", "mute", "auto_input", "shortcut_1", "shortcut_2", "language",
            "screen_dimming", "taskbar_detection", "boundary_detection", "outer_dimming", "global_dimming", "uniform_brightness",
            "crosshair", "timer", "fps_counter", "display_alignment",
        ]);
    }
    let (mut pass, mut fail) = (0, 0);
    for id in tests {
        let Some((orig, tmp)) = pick(id) else {
            println!("skip {id}: not available");
            continue;
        };
        let r = m.apply(id, tmp).and_then(|a| m.apply(id, orig).map(|b| (a, b)));
        match r {
            Ok((a, b)) => {
                pass += 1;
                println!("ok   {id}: {a}, restored {b}");
            }
            Err(e) => {
                fail += 1;
                println!("FAIL {id}: {e:#}");
                // Best effort: put the original value back.
                if let Err(e2) = m.apply(id, orig) {
                    println!("     could not restore {id}={orig}: {e2:#}");
                }
            }
        }
    }
    if extended {
        // Settings that only take writes in a given state: set it up, test, restore.
        let mut staged = |m: &mut sumi_core::Monitor, setup: (&str, u16), tests: &[&str]| {
            let Some((orig, _)) = pick(setup.0) else { return };
            if let Err(e) = m.apply(setup.0, setup.1) {
                fail += 1;
                println!("FAIL {} = {} (setup): {e:#}", setup.0, setup.1);
                return;
            }
            let st = m.state(None);
            for id in tests {
                let Some(f) = st.features.iter().find(|f| f.id == *id && f.supported && f.enabled) else {
                    println!("skip {id}: not enabled after {} = {}", setup.0, setup.1);
                    continue;
                };
                let Some(cur) = f.value else { continue };
                let tmp = if cur > 0 { cur - 1 } else { cur + 1 };
                match m.apply(id, tmp).and_then(|a| m.apply(id, cur).map(|b| (a, b))) {
                    Ok((a, b)) => {
                        pass += 1;
                        println!("ok   {id} (with {} = {}): {a}, restored {b}", setup.0, setup.1);
                    }
                    Err(e) => {
                        fail += 1;
                        println!("FAIL {id}: {e:#}");
                        let _ = m.apply(id, cur);
                    }
                }
            }
            match m.apply(setup.0, orig) {
                Ok(s) => println!("     restored {s}"),
                Err(e) => println!("     could not restore {}={orig}: {e:#}", setup.0),
            }
        };
        staged(m, ("color_temp", 0x0B), &["red_gain", "green_gain", "blue_gain"]);
        staged(m, ("gamevisual", 0x05), &["blue_light"]);
    }
    println!("{pass} passed, {fail} failed");
    if fail > 0 {
        bail!("write self-test failed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use sumi_core::testing;

    #[test]
    fn hex_codes_take_an_optional_prefix() {
        assert_eq!(parse_hex8("dc"), Ok(0xDC));
        assert_eq!(parse_hex8("0xFD"), Ok(0xFD));
        assert_eq!(parse_hex8("0X10"), Ok(0x10));
        assert!(parse_hex8("100").is_err() && parse_hex8("zz").is_err() && parse_hex8("").is_err());
    }

    #[test]
    fn values_are_decimal_or_0x_hex() {
        assert_eq!(parse_u16("50"), Ok(50));
        assert_eq!(parse_u16("0x7800"), Ok(0x7800));
        assert_eq!(parse_u16("0XFFFF"), Ok(0xFFFF));
        assert!(parse_u16("65536").is_err() && parse_u16("-1").is_err() && parse_u16("7800h").is_err());
    }

    #[test]
    fn the_command_line_is_well_formed() {
        Cli::command().debug_assert();
        // Writes parse, but only run with --confirm (checked when the command runs).
        for args in [["sumi", "apply", "brightness", "50"].as_slice(), &["sumi", "set", "10", "50", "--confirm"], &["sumi", "write-selftest"]] {
            assert!(Cli::try_parse_from(args).is_ok(), "{args:?}");
        }
        assert!(Cli::try_parse_from(["sumi", "set", "10"]).is_err());
        assert!(Cli::try_parse_from(["sumi", "get"]).is_err());
    }

    #[test]
    fn write_selftest_leaves_the_monitor_as_it_found_it() {
        for extended in [false, true] {
            let (mut m, fake) = testing::pg32ucdm3();
            let before = testing::PG32UCDM3_REGS.iter().map(|&(c, _, _)| (c, fake.current(c))).collect::<Vec<_>>();
            write_selftest(&mut m, extended).unwrap();
            // It did write…
            assert!(fake.writes().len() >= 16, "{} writes", fake.writes().len());
            // …every setting went through and came back…
            for (code, was) in before {
                assert_eq!(fake.current(code), was, "0x{code:02X} (extended: {extended})");
            }
            // …and never touched the pixel cleaning bit.
            assert!(fake.writes().iter().all(|&(c, v)| c != 0xFD || v & 0x0010 == 0));
        }
    }

    #[test]
    fn write_selftest_fails_when_the_monitor_ignores_a_write() {
        let (mut m, fake) = testing::pg32ucdm3();
        fake.ignore_writes(0x10);
        let e = write_selftest(&mut m, false).unwrap_err().to_string();
        assert_eq!(e, "write self-test failed");
    }

    #[test]
    fn write_selftest_cannot_write_to_a_read_only_link() {
        let (mut m, fake) = testing::pg32ucdm3();
        m.display.allow_writes(false);
        assert!(write_selftest(&mut m, true).is_err());
        assert!(fake.writes().is_empty());
    }
}
