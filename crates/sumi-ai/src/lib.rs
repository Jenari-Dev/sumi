//! AI Visual: picks a GameVisual mode from what is on screen.
//!
//! `capture` grabs the monitor's output from KWin, `clip` scores it against
//! scene prompts with OpenAI's CLIP (ViT-B/32), and `decision` applies
//! DisplayWidget Center's smoothing and switching rules.

pub mod capture;
pub mod clip;
pub mod decision;
pub mod model;
pub mod preprocess;
pub mod prompts;

pub use decision::{Engine, Mode, Observation, Scene, Settings, Verdict};
