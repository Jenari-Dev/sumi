//! Sumi protocol layer: DDC/CI framing, MCCS capability strings, EDID, and
//! the ASUS DisplayWidget VCP catalogue. No I/O happens here.

pub mod caps;
pub mod ddcci;
pub mod edid;
pub mod vcp;

pub use caps::{CapValue, Capabilities};
pub use ddcci::{FrameError, VcpReply, VcpType};
pub use edid::Edid;
