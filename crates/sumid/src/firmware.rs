//! Firmware update check and download (DisplayWidget's check, ported: see
//! `sumi_core::firmware`). Only ever runs when the app or CLI asks; the daemon never looks on its
//! own. Reading the monitor's firmware version and asking ASUS write nothing to the monitor, so
//! both work with `--read-only`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, bail};
use serde::Serialize;
use sumi_core::firmware::{self, Check};
use tracing::{info, warn};

use crate::manager::Manager;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Download {
    /// "idle" | "running" | "done" | "error"
    pub state: &'static str,
    pub id: String,
    pub done: u64,
    pub total: u64,
    pub path: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Default)]
pub struct FwState {
    pub checks: BTreeMap<String, Check>,
    pub download: Download,
}

impl FwState {
    /// The reply of CheckFirmware: {current, latest|null, url|null, checked_at, …}.
    pub fn record(&mut self, id: &str, c: Check) -> serde_json::Value {
        let v = serde_json::to_value(&c).unwrap_or_default();
        self.checks.insert(id.to_string(), c);
        v
    }

    /// The reply of FirmwareStatus: {checks: {id: <check>}, download: {state, done, total, path?, error?}}.
    pub fn status(&self) -> serde_json::Value {
        serde_json::json!({ "checks": self.checks, "download": self.download })
    }
}

impl Manager {
    fn fw_identity(&self, id: &str) -> Result<(String, u16)> {
        let m = self.get(id)?;
        let m = m.lock().unwrap();
        Ok((m.model().to_string(), m.fw.fw_version))
    }

    /// Ask ASUS (through `head`) whether newer firmware exists for this monitor.
    pub fn firmware_check_with(&self, id: &str, head: impl FnMut(&str) -> Result<u16>) -> Result<serde_json::Value> {
        let (model, fw) = self.fw_identity(id)?;
        if fw == 0 {
            bail!("this monitor didn't report a firmware version");
        }
        info!("firmware check for {model} (MCM{fw})");
        let c = firmware::check(&model, fw, head)?;
        Ok(self.firmware.lock().unwrap().record(id, c))
    }

    pub fn firmware_status(&self) -> serde_json::Value {
        self.firmware.lock().unwrap().status()
    }

    /// Download the zip found by the last check of `id` into `dir`; progress in `firmware_status`.
    /// `fetch(url, dir, progress)` is the injected transfer.
    pub fn firmware_download_with<F>(self: &Arc<Self>, id: &str, dir: PathBuf, fetch: F) -> Result<()>
    where
        F: FnOnce(&str, &std::path::Path, &mut dyn FnMut(u64, u64)) -> Result<PathBuf> + Send + 'static,
    {
        let url = {
            let mut st = self.firmware.lock().unwrap();
            let Some(url) = st.checks.get(id).and_then(|c| c.url.clone()) else {
                bail!("no newer firmware was found: check for updates first");
            };
            if st.download.state == "running" {
                bail!("a firmware download is already running");
            }
            st.download = Download { state: "running", id: id.to_string(), ..Default::default() };
            url
        };
        let mgr = self.clone();
        let id = id.to_string();
        std::thread::spawn(move || {
            info!("downloading {url} to {}", dir.display());
            let r = fetch(&url, &dir, &mut |done, total| {
                let mut st = mgr.firmware.lock().unwrap();
                st.download.done = done;
                st.download.total = total;
            });
            let mut st = mgr.firmware.lock().unwrap();
            match r {
                Ok(p) => {
                    info!("firmware saved to {}", p.display());
                    st.download = Download { state: "done", id, done: st.download.done, total: st.download.total, path: Some(p.display().to_string()), error: None };
                }
                Err(e) => {
                    warn!("firmware download failed: {e:#}");
                    st.download = Download { state: "error", id, error: Some(format!("{e:#}")), ..Default::default() };
                }
            }
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found() -> Check {
        firmware::check("PG32UCDM3", 103, |u| Ok(if u.ends_with("MCM105.zip") { 200 } else { 404 })).unwrap()
    }

    #[test]
    fn check_reply_shape() {
        let mut st = FwState::default();
        let v = st.record("PG32UCDM3@i2c-5", found());
        assert_eq!(v["current"], "MCM103");
        assert_eq!(v["latest"], "MCM105");
        assert_eq!(v["latest_version"], 105);
        assert!(v["url"].as_str().unwrap().ends_with("/PG32UCDM3/ASUS_PG32UCDM3_MCM105.zip"));
        assert!(v["checked_at"].as_u64().unwrap() > 1_600_000_000);
        let up = firmware::check("PG32UCDM3", 103, |_| Ok(404)).unwrap();
        let v = st.record("b", up);
        assert!(v["latest"].is_null() && v["url"].is_null());
    }

    #[test]
    fn status_reply_shape() {
        let mut st = FwState::default();
        st.record("a", found());
        let v = st.status();
        assert_eq!(v["download"]["state"], "");
        assert_eq!(v["checks"]["a"]["latest"], "MCM105");
    }

    fn manager() -> Arc<Manager> {
        Arc::new(Manager::new(true, std::env::temp_dir().join("sumi-fw-test-config.toml")).unwrap())
    }

    fn wait_for(mgr: &Arc<Manager>, state: &str) -> serde_json::Value {
        for _ in 0..200 {
            let v = mgr.firmware_status();
            if v["download"]["state"] == state {
                return v;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("never reached {state}: {}", mgr.firmware_status());
    }

    #[test]
    fn download_needs_a_check_and_reports_progress_and_the_path_even_read_only() {
        let mgr = manager();
        let dir = std::env::temp_dir();
        assert!(mgr.firmware_download_with("a", dir.clone(), |_, _, _| unreachable!()).is_err(), "no check yet");
        mgr.firmware.lock().unwrap().record("a", found());
        mgr.firmware_download_with("a", dir.clone(), |url, d, p| {
            assert!(url.ends_with("MCM105.zip"));
            p(5, 10);
            Ok(d.join("ASUS_PG32UCDM3_MCM105.zip"))
        })
        .unwrap();
        let v = wait_for(&mgr, "done");
        assert_eq!(v["download"]["path"], dir.join("ASUS_PG32UCDM3_MCM105.zip").display().to_string());
        assert_eq!(v["download"]["done"], 5);
        // And a failure is reported, not hidden.
        mgr.firmware_download_with("a", dir, |_, _, _| bail!("Couldn't reach ASUS's download server")).unwrap();
        let v = wait_for(&mgr, "error");
        assert!(v["download"]["error"].as_str().unwrap().contains("Couldn't reach"));
    }
}
