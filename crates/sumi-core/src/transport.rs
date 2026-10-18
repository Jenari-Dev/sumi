//! The DDC/CI channel a `Monitor` talks through. `sumi_ddc::Display` is the
//! real one; tests use the in-memory fake in `testing`.

use std::time::Duration;

use sumi_ddc::Display;
use sumi_proto::VcpReply;

/// What the verified write path needs from a monitor link.
pub trait Transport: Send {
    fn get_vcp(&mut self, code: u8) -> sumi_ddc::Result<VcpReply>;
    /// Fire-and-forget Set; refused until `allow_writes(true)`.
    fn set_vcp(&mut self, code: u8, value: u16) -> sumi_ddc::Result<()>;
    fn capabilities_raw(&mut self) -> sumi_ddc::Result<String>;
    fn allow_writes(&mut self, yes: bool);
    /// The connected outputs' EDIDs as the kernel has them ("DP-3", bytes):
    /// no DDC traffic, but machine state a fake must not leak into tests.
    fn output_edids(&self) -> Vec<(String, Vec<u8>)> {
        sumi_ddc::drm::connector_edids()
    }
    /// Wait for the monitor to settle between a write and its read-back.
    /// A fake overrides this so tests don't sleep.
    fn pause(&mut self, d: Duration) {
        std::thread::sleep(d);
    }
}

impl Transport for Display {
    fn get_vcp(&mut self, code: u8) -> sumi_ddc::Result<VcpReply> {
        Display::get_vcp(self, code)
    }

    fn set_vcp(&mut self, code: u8, value: u16) -> sumi_ddc::Result<()> {
        Display::set_vcp(self, code, value)
    }

    fn capabilities_raw(&mut self) -> sumi_ddc::Result<String> {
        Display::capabilities_raw(self)
    }

    fn allow_writes(&mut self, yes: bool) {
        Display::allow_writes(self, yes)
    }
}
