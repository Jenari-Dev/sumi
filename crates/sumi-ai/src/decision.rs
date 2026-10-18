//! AI Visual's decision logic, as DisplayWidget Center implements it
//! (`AppSyncPage.OnInflurenceRawProcess` + `AISwitchHelper`):
//!
//! 1. Raw class scores are shifted so the minimum is 0, the nine scenery
//!    sub-classes are dropped (only the "Scenery" class counts), and the rest
//!    are normalised to sum 100 ("self-softmax").
//! 2. Each class is smoothed with its own EMA (α 0.55–0.65); the two gates
//!    (is it a game, is the model confident) with α 0.6.
//! 3. The top class wins; with both gates over 50 % its mode is applied. When
//!    the gates disagree, game modes need the game gate and non-game modes the
//!    confidence gate; otherwise the user's fallback mode applies.
//! 4. A mode is applied only if the user enabled it for AI Visual, else the
//!    fallback. Nothing is re-sent while the target stays the same.

use serde::Serialize;

/// The scene classes, in DisplayWidget's order. `Scenery` sub-classes are
/// folded into `Scenery`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scene {
    Fps,
    Moba,
    Office,
    Racing,
    Rpg,
    Scenery,
}

impl Scene {
    pub const ALL: [Scene; 6] = [Scene::Fps, Scene::Moba, Scene::Office, Scene::Racing, Scene::Rpg, Scene::Scenery];

    /// GameVisual (0xDC) mode the scene maps to.
    pub fn mode(self) -> Mode {
        match self {
            Scene::Fps => Mode::Fps,
            Scene::Moba => Mode::Moba,
            Scene::Office => Mode::Srgb,
            Scene::Racing => Mode::Racing,
            Scene::Rpg => Mode::RtsRpg,
            Scene::Scenery => Mode::Scenery,
        }
    }

    /// EMA weight of the newest score (DisplayWidget's per-class alpha).
    pub fn alpha(self) -> f64 {
        match self {
            Scene::Moba => 0.65,
            Scene::Office | Scene::Rpg => 0.55,
            _ => 0.6,
        }
    }

    fn is_game(self) -> bool {
        matches!(self, Scene::Fps | Scene::Moba | Scene::Racing | Scene::Rpg)
    }
}

/// GameVisual modes AI Visual can pick (DisplayWidget's `SplendidGroup`s).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Cinema,
    Scenery,
    Srgb,
    User,
    Racing,
    RtsRpg,
    Fps,
    Moba,
}

impl Mode {
    /// The 0xDC value; sRGB depends on the monitor (3, or 0x0A with sRGB calibration).
    pub fn vcp_value(self, srgb: u16) -> u16 {
        match self {
            Mode::Cinema => 1,
            Mode::Scenery => 2,
            Mode::Srgb => srgb,
            Mode::User => 4,
            Mode::Racing => 5,
            Mode::RtsRpg => 6,
            Mode::Fps => 7,
            Mode::Moba => 8,
        }
    }
}

/// One classifier result: raw per-scene scores (any scale) and the two gates in 0..1.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Observation {
    pub scores: Vec<(Scene, f64)>,
    pub is_game: f64,
    pub confidence: f64,
}

/// User settings (DisplayWidget's `DisplayAiVisual.json`).
#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Modes AI Visual may switch to. DisplayWidget default: RTS/RPG, FPS,
    /// Scenery, Racing, sRGB and MOBA on; Cinema and User off.
    pub enabled_modes: Vec<Mode>,
    /// "Assign my preference": mode for scenes that aren't enabled or certain.
    /// `None` = "Remain unchanged".
    pub fallback: Option<Mode>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            enabled_modes: vec![Mode::RtsRpg, Mode::Fps, Mode::Scenery, Mode::Racing, Mode::Srgb, Mode::Moba],
            fallback: None,
        }
    }
}

/// The smoothed view after an observation, for the UI's score display.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Verdict {
    /// Smoothed, normalised scores in percent, highest first.
    pub scores: Vec<(Scene, f64)>,
    pub is_game: f64,
    pub confidence: f64,
    pub top: Option<Scene>,
    /// The mode AI Visual wants now (None = leave the monitor alone).
    pub target: Option<Mode>,
}

#[derive(Debug, Clone, Default)]
pub struct Engine {
    smoothed: Vec<(Scene, f64)>,
    is_game: f64,
    confidence: f64,
    last_sent: Option<Mode>,
}

const GATE_ALPHA: f64 = 0.6;

/// Shift to a zero minimum and scale to sum 100 (DisplayWidget's "self-softmax").
pub fn normalise(raw: &[(Scene, f64)]) -> Vec<(Scene, f64)> {
    let min = raw.iter().map(|(_, s)| *s).fold(0.0f64, f64::min);
    let shifted: Vec<(Scene, f64)> = raw.iter().map(|(c, s)| (*c, s + min.abs())).collect();
    let sum: f64 = shifted.iter().map(|(_, s)| *s).filter(|s| *s > 0.0).sum();
    shifted.into_iter().map(|(c, s)| (c, if sum > 0.0 && s != 0.0 { s / sum * 100.0 } else { 0.0 })).collect()
}

impl Engine {
    pub fn new() -> Self {
        Engine::default()
    }

    /// Forget smoothing and the last mode sent (AI Visual switched on/off).
    pub fn reset(&mut self) {
        *self = Engine::default();
    }

    /// The mode last handed to the monitor by this engine.
    pub fn last_sent(&self) -> Option<Mode> {
        self.last_sent
    }

    /// Feed one observation. Returns the verdict and, if the monitor should
    /// change, the mode to send now.
    pub fn observe(&mut self, obs: &Observation, settings: &Settings) -> (Verdict, Option<Mode>) {
        let norm = normalise(&obs.scores);
        for (scene, score) in norm {
            match self.smoothed.iter_mut().find(|(c, _)| *c == scene) {
                Some((_, prev)) => *prev = scene.alpha() * score + (1.0 - scene.alpha()) * *prev,
                None => self.smoothed.push((scene, scene.alpha() * score)),
            }
        }
        self.is_game = GATE_ALPHA * obs.is_game + (1.0 - GATE_ALPHA) * self.is_game;
        self.confidence = GATE_ALPHA * obs.confidence + (1.0 - GATE_ALPHA) * self.confidence;

        let mut ranked = self.smoothed.clone();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
        let top = ranked.first().filter(|(_, s)| *s > 0.0).map(|(c, _)| *c);
        let game = self.is_game > 0.5;
        let confident = self.confidence > 0.5;

        // DisplayWidget's decision table.
        let use_top = match (confident, game, top) {
            (_, _, None) => false,
            (true, true, Some(_)) => true,
            (true, false, Some(t)) => !t.is_game(),
            (false, true, Some(t)) => t.is_game(),
            (false, false, _) => false,
        };
        let target = match top {
            Some(t) if use_top && settings.enabled_modes.contains(&t.mode()) => Some(t.mode()),
            _ => settings.fallback,
        };
        let send = target.filter(|m| Some(*m) != self.last_sent);
        if send.is_some() {
            self.last_sent = send;
        }
        (Verdict { scores: ranked, is_game: self.is_game, confidence: self.confidence, top, target }, send)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(top: Scene, is_game: f64, confidence: f64) -> Observation {
        Observation {
            scores: Scene::ALL.iter().map(|&c| (c, if c == top { 8.0 } else { 1.0 })).collect(),
            is_game,
            confidence,
        }
    }

    #[test]
    fn normalise_matches_vendor() {
        let n = normalise(&[(Scene::Fps, 3.0), (Scene::Moba, -1.0), (Scene::Office, 0.0)]);
        // min -1 → shifted 4, 0, 1 → 80 %, 0, 20 %.
        assert_eq!(n, vec![(Scene::Fps, 80.0), (Scene::Moba, 0.0), (Scene::Office, 20.0)]);
    }

    #[test]
    fn game_with_both_gates_switches_once() {
        let mut e = Engine::new();
        let s = Settings::default();
        let (_, first) = e.observe(&obs(Scene::Fps, 1.0, 1.0), &s);
        assert_eq!(first, Some(Mode::Fps));
        // Same scene again: nothing is re-sent.
        let (v, again) = e.observe(&obs(Scene::Fps, 1.0, 1.0), &s);
        assert_eq!(again, None);
        assert_eq!(v.target, Some(Mode::Fps));
    }

    #[test]
    fn gates_need_smoothing_to_open() {
        let mut e = Engine::new();
        let s = Settings::default();
        // First frame: EMA gates reach 0.6 · 0.8 = 0.48 < 0.5 → fallback (none).
        let (_, sent) = e.observe(&obs(Scene::Racing, 0.8, 0.8), &s);
        assert_eq!(sent, None);
        let (_, sent) = e.observe(&obs(Scene::Racing, 0.8, 0.8), &s);
        assert_eq!(sent, Some(Mode::Racing));
    }

    #[test]
    fn office_needs_confidence_not_game() {
        let mut e = Engine::new();
        let s = Settings::default();
        for _ in 0..3 {
            e.observe(&obs(Scene::Office, 0.0, 1.0), &s);
        }
        assert_eq!(e.last_sent(), Some(Mode::Srgb));
        // A game scene with the game gate closed falls back (none = unchanged).
        let mut e = Engine::new();
        for _ in 0..3 {
            e.observe(&obs(Scene::Fps, 0.0, 1.0), &s);
        }
        assert_eq!(e.last_sent(), None);
    }

    #[test]
    fn disabled_modes_use_the_fallback() {
        let mut e = Engine::new();
        let s = Settings { enabled_modes: vec![Mode::Fps], fallback: Some(Mode::User) };
        for _ in 0..3 {
            e.observe(&obs(Scene::Moba, 1.0, 1.0), &s);
        }
        assert_eq!(e.last_sent(), Some(Mode::User));
    }

    #[test]
    fn srgb_value_follows_the_monitor() {
        assert_eq!(Mode::Srgb.vcp_value(0x0A), 0x0A);
        assert_eq!(Mode::Fps.vcp_value(0x0A), 7);
    }
}
