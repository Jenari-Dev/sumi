//! sumid — the Sumi monitor daemon. Owns the DDC/CI buses, notices monitor
//! hotplug and OSD-side changes, and serves the D-Bus API.

mod ai;
mod appsync;
mod dbus;
mod firmware;
mod desktop;
mod gamepad;
mod i18n;
mod kwin;
mod manager;
mod shortcuts;
mod tiling;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use tracing::info;

/// The desktop app's single-instance name; the poll runs only while it is owned.
pub(crate) const APP_NAME: &str = "io.github.JenAriDev.Sumi.App";

#[derive(Parser)]
#[command(version, about = "Sumi monitor daemon")]
struct Args {
    /// Never write to monitors (state, polling and D-Bus reads still work).
    #[arg(long)]
    read_only: bool,
    /// Seconds between polls of the mode/HDR/power registers (0 = off).
    #[arg(long, default_value_t = 5)]
    poll: u64,
    /// Config file (default: ~/.config/sumi/config.toml).
    #[arg(long)]
    config: Option<std::path::PathBuf>,
    /// Don't look for monitors (desktop-integration tests without DDC access).
    #[arg(long)]
    no_monitors: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_target(false).with_env_filter(filter).init();
    let args = Args::parse();
    i18n::init();
    let config_path = args.config.unwrap_or_else(sumi_core::Config::default_path);
    let mgr = Arc::new(manager::Manager::new(args.read_only, config_path)?);

    let conn = zbus::connection::Builder::session()?
        .name(dbus::BUS_NAME)?
        .serve_at(dbus::PATH, dbus::Sumi { mgr: mgr.clone() })?
        .build()
        .await
        .context("D-Bus: is another sumid already running?")?;
    let iface = conn.object_server().interface::<_, dbus::Sumi>(dbus::PATH).await?;
    info!("serving {} on the session bus{}", dbus::BUS_NAME, if mgr.read_only { " (read-only)" } else { "" });

    // KDE integration: hot keys and the App Sync window watcher (skipped
    // gracefully outside a Plasma session, e.g. on a private test bus).
    if !mgr.read_only {
        shortcuts::spawn(mgr.clone());
        gamepad::spawn(mgr.clone());
    }
    ai::spawn(mgr.clone());
    let kwin_loaded = tokio::task::spawn_blocking(|| {
        let conn = zbus::blocking::Connection::session()?;
        kwin::load_appsync(&conn)
    })
    .await
    .map_err(anyhow::Error::from)
    .and_then(|r| r)
    .map_err(|e| tracing::warn!("App Sync window watcher not loaded: {e:#}"))
    .is_ok();

    // Hotplug: watch DRM connector status (a sysfs read); rescan buses on change.
    let no_monitors = args.no_monitors;
    let hot = {
        let mgr = mgr.clone();
        let iface = iface.clone();
        tokio::spawn(async move {
            loop {
                if no_monitors {
                    tokio::time::sleep(Duration::from_secs(3600)).await;
                    continue;
                }
                let m = mgr.clone();
                let changed = tokio::task::spawn_blocking(move || m.connectors_changed()).await.unwrap_or(false);
                if changed {
                    // Give the link and the monitor's DDC controller time to come up.
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    let m = mgr.clone();
                    let (added, removed) = tokio::task::spawn_blocking(move || m.refresh()).await.unwrap_or_default();
                    for id in added {
                        let _ = dbus::Sumi::monitor_added(iface.signal_emitter(), &id).await;
                    }
                    for id in removed {
                        let _ = dbus::Sumi::monitor_removed(iface.signal_emitter(), &id).await;
                    }
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        })
    };

    // OSD-side changes: poll the few registers that drive the UI's enable rules,
    // but only while the Sumi app is running (DisplayWidget polls only while its
    // window is open). Each DDC read costs the NVIDIA driver ~75 ms of CPU.
    let poll = (args.poll > 0).then(|| {
        let mgr = mgr.clone();
        let iface = iface.clone();
        let every = Duration::from_secs(args.poll);
        let dbus = conn.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(every).await;
                let app_running = dbus
                    .call_method(Some("org.freedesktop.DBus"), "/org/freedesktop/DBus", Some("org.freedesktop.DBus"), "NameHasOwner", &(APP_NAME,))
                    .await
                    .ok()
                    .and_then(|m| m.body().deserialize::<bool>().ok())
                    .unwrap_or(true);
                if !app_running {
                    continue;
                }
                let m = mgr.clone();
                let changed = tokio::task::spawn_blocking(move || m.poll()).await.unwrap_or_default();
                for (id, w) in changed {
                    let json = serde_json::to_string(&w).unwrap_or_default();
                    info!("{id}: changed on the monitor: {json}");
                    let _ = dbus::Sumi::state_changed(iface.signal_emitter(), &id, &json).await;
                }
            }
        })
    });

    // systemd stops us with SIGTERM; Ctrl+C when run by hand.
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        r = tokio::signal::ctrl_c() => r?,
        _ = term.recv() => {}
    }
    info!("shutting down");
    if kwin_loaded {
        let _ = tokio::task::spawn_blocking(|| zbus::blocking::Connection::session().map(|c| kwin::unload_appsync(&c))).await;
    }
    hot.abort();
    if let Some(p) = poll {
        p.abort();
    }
    Ok(())
}
