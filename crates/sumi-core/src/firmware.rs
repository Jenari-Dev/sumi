//! DisplayWidget's firmware update check, ported (DisplayFWUpdateManager.CheckIsFileExist, the
//! non-ProArt branch): ASUS publishes `ASUS_<model>_MCM<v>.zip` under a fixed URL; the check asks
//! with HEAD for the ten versions after the current one and takes the first that exists.
//!
//! Network only ever happens on an explicit request (`check`, `download`). Sumi never flashes or
//! unpacks anything: the zip holds ASUS's Windows update tool.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Serialize;

pub const ROOT: &str = "https://dlcdnets.asus.com/pub/ASUS/LCD%20Monitors/";
/// Timeout of one HEAD request.
pub const HEAD_TIMEOUT: Duration = Duration::from_secs(5);

/// "MCM103" for a scalar firmware version.
pub fn label(version: u16) -> String {
    format!("MCM{version}")
}

/// Percent-encode a model name for a URL path segment (letters, digits and `-._~` stay).
fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The URLs to try, in the vendor's order: i = 10 down to 1, version ((fw + i) % 100) + 100.
pub fn candidates(model: &str, fw: u16) -> Vec<(u16, String)> {
    let m = encode(model);
    (1..=10u32)
        .rev()
        .map(|i| {
            let v = ((fw as u32 + i) % 100) + 100;
            (v as u16, format!("{ROOT}{m}/ASUS_{m}_MCM{v}.zip"))
        })
        .collect()
}

/// What a check found.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Check {
    pub model: String,
    pub current_version: u16,
    /// "MCM103"
    pub current: String,
    pub latest_version: Option<u16>,
    pub latest: Option<String>,
    pub url: Option<String>,
    /// Unix seconds.
    pub checked_at: u64,
    /// Every URL asked, with its HTTP status.
    pub tried: Vec<(String, u16)>,
}

/// Check with `head(url) -> HTTP status` (injected, so tests never touch the network). A failure
/// to ask at all (no network, timeout) is an error; any HTTP status, 404 included, is an answer.
pub fn check(model: &str, fw: u16, mut head: impl FnMut(&str) -> Result<u16>) -> Result<Check> {
    let mut tried = Vec::new();
    let mut found = None;
    for (v, url) in candidates(model, fw) {
        let status = head(&url)?;
        tried.push((url.clone(), status));
        if status == 200 {
            found = Some((v, url));
            break;
        }
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    Ok(Check {
        model: model.to_string(),
        current_version: fw,
        current: label(fw),
        latest_version: found.as_ref().map(|f| f.0),
        latest: found.as_ref().map(|f| label(f.0)),
        url: found.map(|f| f.1),
        checked_at: now,
        tried,
    })
}

fn agent(timeout: Option<Duration>) -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(timeout).http_status_as_error(false).build().into()
}

/// The real HEAD request. Says "Couldn't reach ASUS's download server" when it can't.
pub fn http_head(url: &str) -> Result<u16> {
    agent(Some(HEAD_TIMEOUT))
        .head(url)
        .call()
        .map(|r| r.status().as_u16())
        .map_err(|e| anyhow::anyhow!("Couldn't reach ASUS's download server ({e})"))
}

/// The user's Download folder (XDG), else ~/Downloads.
pub fn download_dir() -> PathBuf {
    dirs::download_dir().or_else(|| dirs::home_dir().map(|h| h.join("Downloads"))).unwrap_or_else(|| PathBuf::from("."))
}

/// `dir/name`, or `dir/stem (1).ext`, `(2)` … when that exists: nothing is overwritten.
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (name.to_string(), String::new()),
    };
    (1..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .expect("endless range")
}

/// File name of a firmware URL.
pub fn file_name(url: &str) -> &str {
    url.rsplit('/').next().unwrap_or("firmware.zip")
}

/// Stream `reader` into a new file in `dir` (never overwriting), calling `progress(done, total)`.
pub fn save(mut reader: impl Read, total: u64, dir: &Path, name: &str, mut progress: impl FnMut(u64, u64)) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let dest = unique_path(dir, name);
    let part = dest.with_extension("zip.part");
    let mut out = std::fs::File::create(&part)?;
    let mut buf = vec![0u8; 1 << 16];
    let mut done = 0u64;
    loop {
        let n = reader.read(&mut buf).context("reading the download")?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        done += n as u64;
        progress(done, total.max(done));
    }
    out.sync_all()?;
    if total > 0 && done != total {
        let _ = std::fs::remove_file(&part);
        bail!("the download stopped after {done} of {total} bytes");
    }
    std::fs::rename(&part, &dest)?;
    Ok(dest)
}

/// Download `url` into `dir` (the real GET).
pub fn download(url: &str, dir: &Path, progress: impl FnMut(u64, u64)) -> Result<PathBuf> {
    let resp = agent(None).get(url).call().map_err(|e| anyhow::anyhow!("Couldn't reach ASUS's download server ({e})"))?;
    if resp.status().as_u16() != 200 {
        bail!("ASUS's download server answered {}", resp.status());
    }
    let total = resp.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(0);
    save(resp.into_body().into_reader(), total, dir, file_name(url), progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_follow_the_vendor_order() {
        let c = candidates("PG32UCDM3", 103);
        let v: Vec<u16> = c.iter().map(|c| c.0).collect();
        assert_eq!(v, vec![113, 112, 111, 110, 109, 108, 107, 106, 105, 104]);
        assert_eq!(c[0].1, "https://dlcdnets.asus.com/pub/ASUS/LCD%20Monitors/PG32UCDM3/ASUS_PG32UCDM3_MCM113.zip");
    }

    #[test]
    fn versions_wrap_like_the_vendor_math() {
        // ((199 + i) % 100) + 100: 209 % 100 = 9 -> 109 ... 200 % 100 = 0 -> 100
        let v: Vec<u16> = candidates("X", 199).iter().map(|c| c.0).collect();
        assert_eq!(v, vec![109, 108, 107, 106, 105, 104, 103, 102, 101, 100]);
        let v: Vec<u16> = candidates("X", 0).iter().map(|c| c.0).collect();
        assert_eq!(v[0], 110);
        assert_eq!(v[9], 101);
    }

    #[test]
    fn the_model_is_url_encoded() {
        let c = candidates("PG 32/U+CDM3", 103);
        assert!(c[0].1.contains("/PG%2032%2FU%2BCDM3/ASUS_PG%2032%2FU%2BCDM3_MCM113.zip"), "{}", c[0].1);
        assert!(!candidates("Ünï", 1)[0].1.contains('Ü'));
    }

    #[test]
    fn first_hit_wins_and_stops_asking() {
        let mut asked = Vec::new();
        let r = check("PG32UCDM3", 103, |u| {
            asked.push(u.to_string());
            Ok(if u.ends_with("MCM110.zip") || u.ends_with("MCM106.zip") { 200 } else { 404 })
        })
        .unwrap();
        assert_eq!(r.latest_version, Some(110));
        assert_eq!(r.latest.as_deref(), Some("MCM110"));
        assert_eq!(r.current, "MCM103");
        assert!(r.url.unwrap().ends_with("ASUS_PG32UCDM3_MCM110.zip"));
        assert_eq!(asked.len(), 4, "113, 112, 111, 110");
        assert_eq!(r.tried.len(), 4);
        assert_eq!(r.tried[3].1, 200);
    }

    #[test]
    fn none_found_means_up_to_date() {
        let r = check("PG32UCDM3", 103, |_| Ok(404)).unwrap();
        assert_eq!((r.latest, r.url), (None, None));
        assert_eq!(r.tried.len(), 10);
    }

    #[test]
    fn a_network_failure_is_an_error_not_up_to_date() {
        let e = check("PG32UCDM3", 103, |_| Err(anyhow::anyhow!("Couldn't reach ASUS's download server"))).unwrap_err();
        assert!(e.to_string().contains("Couldn't reach"));
    }

    #[test]
    fn saving_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("sumi-fw-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut seen = Vec::new();
        let a = save(&b"abcdef"[..], 6, &dir, "ASUS_X_MCM110.zip", |d, t| seen.push((d, t))).unwrap();
        let b = save(&b"123"[..], 3, &dir, "ASUS_X_MCM110.zip", |_, _| {}).unwrap();
        let c = save(&b"4"[..], 1, &dir, "ASUS_X_MCM110.zip", |_, _| {}).unwrap();
        assert_eq!(a.file_name().unwrap(), "ASUS_X_MCM110.zip");
        assert_eq!(b.file_name().unwrap(), "ASUS_X_MCM110 (1).zip");
        assert_eq!(c.file_name().unwrap(), "ASUS_X_MCM110 (2).zip");
        assert_eq!(std::fs::read(&a).unwrap(), b"abcdef");
        assert_eq!(seen.last(), Some(&(6, 6)));
        assert!(save(&b"ab"[..], 5, &dir, "short.zip", |_, _| {}).is_err(), "a short download is refused");
        assert!(!dir.join("short.zip").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_name_is_the_last_segment() {
        assert_eq!(file_name("https://x/y/ASUS_A_MCM1.zip"), "ASUS_A_MCM1.zip");
    }
}
