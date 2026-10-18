//! The feature catalogue: every monitor setting Sumi exposes, its VCP
//! encoding, how support is decided from the capability string, and when the
//! control is usable (DisplayWidget's per-GameVisual-mode rules).
//!
//! Encodings and rules come from the static RE notes in
//! `asus-displaywidget/notes/` (vcp-picture, vcp-oled-gameplus-system,
//! capability-decoding). Scope: ASUS "new VCP" scheme (0xEF major 2).

use serde::Serialize;
use sumi_proto::{Capabilities, VcpReply};

use crate::monitor::ProductLine;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Page {
    GameVisual,
    GamePlus,
    OledCare,
    MultiScreen,
    System,
    DeviceInfo,
}

/// How a feature's value maps onto the 16-bit VCP word.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Encoding {
    /// 0..=max, the whole word.
    Range,
    /// One of a fixed set of whole-word values.
    Choice,
    /// One of a fixed set of values in the high byte; the low byte is preserved.
    HighByte,
    /// One of a fixed set of values in the low byte; the high byte is preserved.
    LowByte,
    /// One bit of a packed register (read-modify-write).
    Bit { mask: u16 },
    /// Write-only: setting the bit starts an action; the monitor clears it.
    Trigger { mask: u16 },
    /// Write-only command value (OSD keys, nudges, resets).
    Command,
    /// Read-only information.
    Info,
}

/// What a write does beyond changing the setting; the UI confirms anything but `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    None,
    /// Long-running panel maintenance; the screen goes dark.
    PanelMaintenance,
    /// Display disconnects/reconnects (layout changes).
    Replug,
    /// The monitor's USB hub moves to another host.
    MovesUsb,
    /// The monitor shows another input; this PC loses the picture.
    SwitchesInput,
    /// Resets settings.
    Resets,
    /// Locks most monitor features until turned off.
    LocksFeatures,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Opt {
    pub value: u16,
    pub label: &'static str,
}

const fn o(value: u16, label: &'static str) -> Opt {
    Opt { value, label }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Feature {
    pub id: &'static str,
    pub label: &'static str,
    pub page: Page,
    pub code: u8,
    pub encoding: Encoding,
    /// Candidate values (filtered against the capability string per monitor).
    #[serde(skip_serializing_if = "<[Opt]>::is_empty")]
    pub options: &'static [Opt],
    pub risk: Risk,
    /// Shown by DisplayWidget on Gaming models. `false` = an OSD setting Sumi
    /// exposes in addition (labelled as such in the UI).
    pub vendor_ui: bool,
}

const fn f(id: &'static str, label: &'static str, page: Page, code: u8, encoding: Encoding) -> Feature {
    Feature { id, label, page, code, encoding, options: &[], risk: Risk::None, vendor_ui: true }
}

impl Feature {
    const fn opts(mut self, options: &'static [Opt]) -> Self {
        self.options = options;
        self
    }
    const fn risk(mut self, risk: Risk) -> Self {
        self.risk = risk;
        self
    }
    const fn extra(mut self) -> Self {
        self.vendor_ui = false;
        self
    }
}

pub const GAMEVISUAL_MODES: &[Opt] = &[
    o(0x06, "RTS/RPG"),
    o(0x07, "FPS"),
    o(0x01, "Cinema"),
    o(0x02, "Scenery"),
    o(0x05, "Racing"),
    o(0x03, "sRGB"),
    o(0x0A, "sRGB"),
    o(0x08, "MOBA"),
    o(0x04, "User"),
    o(0x09, "Night Vision"),
];

pub const COLOR_TEMPS: &[Opt] = &[
    o(0x03, "4000K"),
    o(0x04, "5000K"),
    o(0x0E, "5500K"),
    o(0x0F, "6000K"),
    o(0x05, "6500K"),
    o(0x10, "7000K"),
    o(0x06, "7500K"),
    o(0x11, "8000K"),
    o(0x07, "8200K"),
    o(0x08, "9300K"),
    o(0x09, "10000K"),
    o(0x0B, "User"),
];

/// 0x72: gamma in the high byte, (v + 100) / 100.
pub const GAMMAS: &[Opt] = &[o(0x50, "1.8"), o(0x64, "2.0"), o(0x78, "2.2"), o(0x8C, "2.4"), o(0xA0, "2.6")];

pub const SHADOW_BOOST: &[Opt] = &[o(0, "Off"), o(1, "Level 1"), o(2, "Level 2"), o(3, "Level 3"), o(4, "Dynamic")];

/// 0xE2 written as a whole word: high byte = family (1 HDR10, 2 Dolby Vision).
pub const HDR_MODES: &[Opt] = &[
    o(0x0101, "Cinema HDR"),
    o(0x0102, "Gaming HDR"),
    o(0x0103, "Console HDR"),
    o(0x0104, "True Black 400"),
    o(0x0108, "True Black 500"),
    o(0x0205, "Dolby Vision Bright"),
    o(0x0206, "Dolby Vision Dark"),
    o(0x0207, "Dolby Vision Gaming"),
    // DisplayWidget "Source only" (DolbySoftware, 520): the source does the Dolby Vision mapping.
    o(0x0208, "Dolby Vision Source only"),
];

pub const CROSSHAIRS: &[Opt] = &[
    o(0, "Off"),
    o(1, "Red Dot"),
    o(2, "Green Dot"),
    o(3, "Red Bullseye"),
    o(4, "Green Bullseye"),
    o(5, "Red Rangefinder"),
    o(6, "Green Rangefinder"),
    o(7, "Blue Dot"),
    o(8, "Green Dot"),
    o(9, "Blue Mini Duplex"),
    o(10, "Green Mini Duplex"),
    o(11, "Blue Heavy Duplex"),
    o(12, "Green Heavy Duplex"),
    o(13, "Style 13"),
    o(14, "Style 14"),
    o(15, "Style 15"),
];

pub const TIMERS: &[Opt] = &[o(0, "Off"), o(1, "30:00"), o(2, "40:00"), o(3, "50:00"), o(4, "60:00"), o(5, "90:00")];
pub const FPS_COUNTER: &[Opt] = &[o(0, "Off"), o(1, "Number"), o(2, "Bar Graph")];
pub const ALIGNMENT: &[Opt] = &[o(0, "Off"), o(1, "On")];
pub const NUDGE: &[Opt] = &[o(1, "Up"), o(2, "Down"), o(3, "Right"), o(4, "Left")];

pub const REMINDER: &[Opt] = &[o(0, "Off"), o(2, "2 hours"), o(4, "4 hours"), o(8, "8 hours")];
pub const SCREEN_MOVE: &[Opt] = &[o(0, "Off"), o(1, "Light"), o(2, "Middle"), o(3, "Strong")];

pub const POWER_SAVING: &[Opt] = &[o(0, "Standard"), o(1, "Power saving")];

pub const INPUTS: &[Opt] = &[
    o(0x0F, "DisplayPort"),
    o(0x10, "DisplayPort 2"),
    o(0x11, "HDMI-1"),
    o(0x12, "HDMI-2"),
    o(0x13, "HDMI-3"),
    o(0x14, "HDMI-4"),
    o(0x1A, "USB-C"),
    o(0x1B, "USB-C 2"),
];

pub const KVM: &[Opt] = &[o(1, "Smart KVM"), o(2, "USB upstream B"), o(3, "USB upstream C"), o(4, "Auto KVM")];

pub const PIP_MODES: &[Opt] = &[
    o(0, "Off"),
    o(1, "PIP top right"),
    o(2, "PIP top left"),
    o(3, "PIP bottom right"),
    o(4, "PIP bottom left"),
    o(5, "PBP 50/50"),
    o(6, "PBP wide left"),
    o(7, "PBP wide right"),
];

pub const SHORTCUTS: &[Opt] = &[
    o(0x02, "GameVisual"),
    o(0x01, "GamePlus"),
    o(0x03, "Brightness"),
    o(0x06, "Contrast"),
    o(0x05, "Shadow Boost"),
    o(0x09, "Blue Light Filter"),
    o(0x0A, "Color Temp."),
    o(0x0B, "Volume"),
    o(0x07, "Input Select"),
    o(0x08, "HDR"),
    o(0x04, "Mute"),
    o(0x0C, "KVM"),
    o(0x0D, "Smart KVM"),
    o(0x0E, "Frame Rate Boost"),
    o(0x0F, "Pixel Cleaning"),
    o(0x10, "Customized Settings-1"),
    o(0x11, "Customized Settings-2"),
];

pub const LANGUAGES: &[Opt] = &[
    o(2, "English"),
    o(3, "Français"),
    o(4, "Deutsch"),
    o(10, "Español"),
    o(5, "Italiano"),
    o(20, "Nederlands"),
    o(9, "Русский"),
    o(30, "Polski"),
    o(18, "Čeština"),
    o(17, "Hrvatski"),
    o(26, "Magyar"),
    o(31, "Română"),
    o(8, "Português"),
    o(12, "Türkçe"),
    o(13, "简体中文"),
    o(1, "繁體中文"),
    o(6, "日本語"),
    o(7, "한국어"),
    o(38, "فارسی"),
    o(35, "ไทย"),
    o(36, "Українська"),
    o(39, "Bahasa Indonesia"),
    o(37, "Tiếng Việt"),
];

pub const OSD_KEYS: &[Opt] = &[
    o(0, "Close"),
    o(1, "Show"),
    o(2, "Up"),
    o(3, "Down"),
    o(4, "Right"),
    o(5, "Left"),
    o(6, "Enter"),
    o(7, "Back"),
];

pub const RESETS: &[Opt] = &[o(0x01, "Reset current mode"), o(0xFF, "Reset all settings")];
pub const MUTE: &[Opt] = &[o(1, "Muted"), o(2, "Unmuted")];

use Encoding::*;
use Page::*;

pub static FEATURES: &[Feature] = &[
    // GameVisual page: mode strip, Image card, Color card, HDR row.
    f("gamevisual", "GameVisual", GameVisual, 0xDC, LowByte).opts(GAMEVISUAL_MODES),
    // Changing the family (HDR10 ↔ Dolby Vision) swaps the advertised EDID: the display re-plugs.
    f("hdr_mode", "HDR mode", GameVisual, 0xE2, Choice).opts(HDR_MODES).risk(Risk::Replug),
    f("brightness", "Brightness", GameVisual, 0x10, Range),
    f("contrast", "Contrast", GameVisual, 0x12, Range),
    f("overdrive", "Variable OD", GameVisual, 0xE0, Range),
    f("blue_light", "Blue Light Filter", GameVisual, 0xE6, Range),
    f("shadow_boost", "Shadow Boost", GameVisual, 0xE5, Choice).opts(SHADOW_BOOST),
    f("ascr", "ASCR", GameVisual, 0xFD, Bit { mask: 0x0002 }),
    f("saturation", "Saturation", GameVisual, 0x8A, Range),
    f("hue", "Hue", GameVisual, 0x90, Range),
    f("color_temp", "Color Temp.", GameVisual, 0x14, Choice).opts(COLOR_TEMPS),
    f("red_gain", "R", GameVisual, 0x16, Range),
    f("green_gain", "G", GameVisual, 0x18, Range),
    f("blue_gain", "B", GameVisual, 0x1A, Range),
    f("gamma", "Gamma", GameVisual, 0x72, HighByte).opts(GAMMAS).extra(),
    f("sharpness", "Sharpness", GameVisual, 0x87, Range).extra(),
    f("reset", "Reset", GameVisual, 0xEC, Command).opts(RESETS).risk(Risk::Resets),
    // GamePlus: four mutually exclusive overlays + position nudge.
    f("crosshair", "Crosshair", GamePlus, 0xE3, Choice).opts(CROSSHAIRS),
    f("timer", "Timer", GamePlus, 0xE4, Choice).opts(TIMERS),
    f("fps_counter", "FPS Counter", GamePlus, 0xEA, Choice).opts(FPS_COUNTER),
    f("display_alignment", "Display Alignment", GamePlus, 0xE7, Choice).opts(ALIGNMENT),
    f("gameplus_position", "Set GamePlus Position", GamePlus, 0xE8, Command).opts(NUDGE),
    // OLED Care (DisplayWidget order).
    f("screen_dimming", "Screen Dimming Control", OledCare, 0xFD, Bit { mask: 0x0008 }),
    f("logo_detection", "Logo Detection", OledCare, 0xFD, Bit { mask: 0x0020 }),
    f("taskbar_detection", "Taskbar Detection", OledCare, 0xFD, Bit { mask: 0x0800 }),
    f("boundary_detection", "Boundary Detection", OledCare, 0xFD, Bit { mask: 0x1000 }),
    f("outer_dimming", "Outer Dimming Control", OledCare, 0xFD, Bit { mask: 0x2000 }),
    f("global_dimming", "Global Dimming Control", OledCare, 0xFD, Bit { mask: 0x4000 }),
    f("uniform_brightness", "Uniform Brightness", OledCare, 0xFD, Bit { mask: 0x0040 }),
    f("pixel_cleaning", "Pixel Cleaning", OledCare, 0xFD, Trigger { mask: 0x0010 }).risk(Risk::PanelMaintenance),
    f("pixel_cleaning_reminder", "Pixel Cleaning Reminder", OledCare, 0xF8, Choice).opts(REMINDER),
    f("screen_move", "Screen Move", OledCare, 0xF9, Choice).opts(SCREEN_MOVE),
    // MultiScreen: PIP/PBP + KVM (MultiFrame is host-side).
    f("pip_mode", "PIP/PBP", MultiScreen, 0xF4, Choice).opts(PIP_MODES).risk(Risk::Replug),
    f("pip_source_a", "Window A source", MultiScreen, 0xF5, LowByte).opts(INPUTS),
    f("pip_source_b", "Window B source", MultiScreen, 0xF5, HighByte).opts(INPUTS),
    f("pip_gamevisual_a", "Window A GameVisual", MultiScreen, 0xF6, LowByte).opts(GAMEVISUAL_MODES),
    f("pip_gamevisual_b", "Window B GameVisual", MultiScreen, 0xF6, HighByte).opts(GAMEVISUAL_MODES),
    f("kvm", "KVM", MultiScreen, 0xF3, Choice).opts(KVM).risk(Risk::MovesUsb),
    // System settings.
    f("power_saving", "Power Settings", System, 0xE1, Choice).opts(POWER_SAVING).risk(Risk::LocksFeatures),
    f("language", "OSD Language", System, 0xCC, Choice).opts(LANGUAGES),
    f("shortcut_1", "OSD Shortcut 1", System, 0xFB, HighByte).opts(SHORTCUTS),
    f("shortcut_2", "OSD Shortcut 2", System, 0xFB, LowByte).opts(SHORTCUTS),
    f("osd_key", "OSD Control", System, 0xEB, Command).opts(OSD_KEYS),
    f("input", "Input Select", System, 0x60, LowByte).opts(INPUTS).risk(Risk::SwitchesInput).extra(),
    f("volume", "Volume", System, 0x62, Range).extra(),
    f("mute", "Mute", System, 0x8D, Choice).opts(MUTE).extra(),
    f("auto_input", "Auto Input Detection", System, 0xFC, Bit { mask: 0x0010 }).extra(),
    f("vrr", "Adaptive-Sync (VRR)", System, 0xFD, Bit { mask: 0x0001 }).extra(),
    // Device info (read-only).
    f("firmware", "F/W Version", DeviceInfo, 0xFF, Info),
    f("usage_hours", "Display usage time", DeviceInfo, 0xC0, Info).extra(),
    f("vertical_freq", "Refresh rate", DeviceInfo, 0xAE, Info).extra(),
];

pub fn by_id(id: &str) -> Option<&'static Feature> {
    FEATURES.iter().find(|f| f.id == id)
}

/// Facts about one monitor that decide support.
#[derive(Debug, Clone)]
pub struct Profile<'a> {
    pub caps: &'a Capabilities,
    pub line: ProductLine,
    pub model: &'a str,
    pub new_vcp: bool,
}

impl Profile<'_> {
    /// DisplayWidget `Utils.IsOledDevice`: `mq`, or a vg/pg/xg name whose 7th character is `d`/`w`.
    pub fn is_oled(&self) -> bool {
        let m = self.model.to_ascii_lowercase();
        let seventh = m.chars().nth(6);
        m.contains("mq") || (["vg", "pg", "xg"].iter().any(|p| m.contains(p)) && matches!(seventh, Some('d' | 'w')))
    }

    /// The capability word for a mask-style entry like `FD(7879)`.
    fn cap_mask(&self, code: u8) -> u16 {
        self.caps.values(code).first().map(|v| v.value).unwrap_or(0)
    }

    fn lo_tokens(&self, code: u8) -> impl Iterator<Item = u16> + '_ {
        self.caps.values(code).iter().filter(|v| v.digits <= 2).map(|v| v.value)
    }

    fn hi_tokens(&self, code: u8) -> impl Iterator<Item = u16> + '_ {
        self.caps.values(code).iter().filter(|v| v.digits == 4).map(|v| v.value >> 8)
    }

    fn has_lo(&self, code: u8, v: u16) -> bool {
        self.lo_tokens(code).any(|t| t == v)
    }

    /// Is the feature present on this monitor at all?
    pub fn supported(&self, feat: &Feature) -> bool {
        let caps = self.caps;
        if !self.new_vcp {
            // Old-scheme models move HDR/ASCR/power saving to other codes; not handled yet.
            return matches!(feat.code, 0x10 | 0x12 | 0x14 | 0x16 | 0x18 | 0x1A | 0x60 | 0x62 | 0x8A | 0xCC | 0xFF);
        }
        match feat.id {
            // OLED models have neither overdrive nor ASCR (DisplayWidget forces them off).
            "overdrive" => caps.supports(0xE0) && !self.is_oled(),
            "ascr" => self.cap_mask(0xFD) & 0x0002 != 0 && !self.is_oled(),
            "hdr_mode" => caps.supports(0xE2),
            "pixel_cleaning" | "screen_dimming" | "logo_detection" | "uniform_brightness" => {
                self.is_oled() && self.cap_mask(0xFD) & bit_mask(feat) != 0
            }
            "pixel_cleaning_reminder" | "screen_move" => self.is_oled() && caps.supports(feat.code),
            "kvm" => self.lo_tokens(0xF3).any(|v| (1..=4).contains(&v)),
            "pip_source_a" | "pip_source_b" | "pip_gamevisual_a" | "pip_gamevisual_b" => caps.supports(0xF4),
            "gameplus_position" => caps.supports(0xE8) && !self.has_lo(0xE8, 0xFE),
            "auto_input" => self.cap_mask(0xFC) & 0x0010 != 0,
            "usage_hours" | "vertical_freq" | "firmware" => caps.supports(feat.code) || feat.code == 0xFF,
            _ => match feat.encoding {
                Bit { mask } | Trigger { mask } => self.cap_mask(feat.code) & mask != 0,
                _ => caps.supports(feat.code),
            },
        }
    }

    /// Options this monitor accepts, in display order.
    pub fn options(&self, feat: &Feature) -> Vec<Opt> {
        let keep = |v: u16| -> bool {
            match feat.id {
                // sRGB is 0x0A on monitors with sRGB calibration, 0x03 otherwise.
                "gamevisual" | "pip_gamevisual_a" | "pip_gamevisual_b" => {
                    let code = if feat.id == "gamevisual" { 0xDC } else { 0xF6 };
                    self.has_lo(code, v) && !(v == 0x03 && self.has_lo(code, 0x0A))
                }
                "hdr_mode" => {
                    let fam = v >> 8;
                    let sub = v & 0xFF;
                    self.hi_tokens(0xE2).any(|t| t == fam) && self.has_lo(0xE2, sub)
                }
                "gamma" => self.hi_tokens(0x72).any(|t| t == v),
                "crosshair" => {
                    // Monitors use either the old (1-6) or the new (7-12+) style set.
                    let new = self.has_lo(0xE3, 0x07) || self.has_lo(0xE3, 0x0C);
                    let in_set = v == 0 || if new { v >= 7 } else { v <= 6 };
                    in_set && self.has_lo(0xE3, v)
                }
                "pip_source_a" | "pip_source_b" => self.has_lo(0x60, v),
                "shortcut_1" | "shortcut_2" => self.has_lo(0xFB, v),
                "pip_mode" | "kvm" | "language" | "color_temp" | "shadow_boost" | "timer" | "fps_counter"
                | "display_alignment" | "gameplus_position" | "reset" | "input" | "power_saving"
                | "pixel_cleaning_reminder" | "screen_move" | "osd_key" | "mute" => {
                    self.caps.values(feat.code).is_empty() || self.has_lo(feat.code, v)
                }
                _ => true,
            }
        };
        feat.options.iter().copied().filter(|o| keep(o.value)).collect()
    }
}

pub fn bit_mask(feat: &Feature) -> u16 {
    match feat.encoding {
        Bit { mask } | Trigger { mask } => mask,
        _ => 0,
    }
}

/// Bits of a packed register that start an action (pixel cleaning). They read
/// 1 while it runs and must never be written back by a read-modify-write.
pub fn trigger_bits(code: u8) -> u16 {
    FEATURES
        .iter()
        .filter(|f| f.code == code)
        .map(|f| if let Trigger { mask } = f.encoding { mask } else { 0 })
        .fold(0, |a, b| a | b)
}

/// Live monitor state that decides which controls are usable right now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Context {
    /// GameVisual mode (0xDC low byte).
    pub mode: u8,
    /// 0xE2 non-zero: the monitor is showing an HDR signal.
    pub hdr: bool,
    /// 0xE1 == 1.
    pub power_saving: bool,
    /// 0x14 == User.
    pub user_color: bool,
    /// Blue Light Filter at its maximum level.
    pub blf_max: bool,
}

/// 0xE2 reports an HDR signal: non-zero, and not the 0xFE "not applicable" value.
pub fn hdr_active(e2: u16) -> bool {
    e2 != 0 && e2 != 0xFE
}

impl Context {
    /// Decode the context registers (0xDC, 0xE2, 0xE1, 0x14, 0xE6) from
    /// whatever `reg` could read. `new_vcp`: 0xE1 is power saving only on the
    /// new scheme (the old one keeps ASCR there).
    pub fn from_regs(new_vcp: bool, reg: impl Fn(u8) -> Option<VcpReply>) -> Self {
        let blf = reg(0xE6);
        Context {
            mode: reg(0xDC).map(|r| r.current as u8).unwrap_or(0),
            hdr: reg(0xE2).is_some_and(|r| hdr_active(r.current)),
            power_saving: new_vcp && reg(0xE1).is_some_and(|r| r.current == 1),
            user_color: reg(0x14).is_some_and(|r| r.current == 0x0B),
            blf_max: blf.is_some_and(|r| r.max > 0 && r.current >= r.max),
        }
    }
}

/// DisplayWidget's per-mode enable table (GameVisualPage.ModeHandler) plus
/// the per-series JSON for settings DisplayWidget doesn't show on Gaming.
pub fn enabled(feat: &Feature, cx: &Context) -> bool {
    let id = feat.id;
    if cx.power_saving {
        return matches!(feat.page, Page::System | Page::DeviceInfo | Page::MultiScreen)
            || matches!(id, "brightness" | "contrast");
    }
    if cx.hdr {
        // A mode tile stays clickable: like DisplayWidget, choosing an SDR mode
        // turns HDR off first (sumid does that).
        return match feat.page {
            Page::GameVisual => matches!(id, "hdr_mode" | "color_temp" | "gamevisual"),
            _ => true,
        };
    }
    if feat.page != Page::GameVisual {
        return true;
    }
    let srgb = matches!(cx.mode, 0x03 | 0x0A);
    let moba = cx.mode == 0x08;
    let night = cx.mode == 0x09;
    let racing = cx.mode == 0x05;
    match id {
        "gamevisual" | "reset" => true,
        "hdr_mode" => false,
        "brightness" => !(racing && cx.blf_max),
        "overdrive" => true,
        "contrast" | "color_temp" | "ascr" => !srgb,
        "saturation" => !srgb && !moba && !night,
        "shadow_boost" => !srgb && !moba,
        "blue_light" => racing,
        "red_gain" | "green_gain" | "blue_gain" => !srgb && cx.user_color,
        "gamma" => !srgb,
        "sharpness" => !moba,
        // Hue is greyed in every Gaming mode.
        "hue" => false,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAPS: &str = include_str!("../../../fixtures/pg32ucdm3-capabilities.txt");

    fn profile(caps: &Capabilities) -> Profile<'_> {
        Profile { caps, line: ProductLine::Gaming, model: "PG32UCDM3", new_vcp: true }
    }

    fn labels(p: &Profile, id: &str) -> Vec<&'static str> {
        p.options(by_id(id).unwrap()).iter().map(|o| o.label).collect()
    }

    #[test]
    fn ids_unique() {
        let mut ids: Vec<_> = FEATURES.iter().map(|f| f.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), FEATURES.len());
    }

    #[test]
    fn pg32ucdm3_support() {
        let caps = sumi_proto::caps::parse(CAPS);
        let p = profile(&caps);
        assert!(p.is_oled());
        let sup = |id| p.supported(by_id(id).unwrap());
        for id in ["brightness", "gamevisual", "hdr_mode", "screen_move", "pixel_cleaning", "taskbar_detection", "kvm", "crosshair", "vrr", "auto_input"] {
            assert!(sup(id), "{id} should be supported");
        }
        for id in ["overdrive", "ascr", "hue"] {
            assert!(!sup(id), "{id} should not be supported");
        }
    }

    #[test]
    fn pg32ucdm3_options() {
        let caps = sumi_proto::caps::parse(CAPS);
        let p = profile(&caps);
        assert_eq!(labels(&p, "gamevisual"), ["RTS/RPG", "FPS", "Cinema", "Scenery", "Racing", "sRGB", "MOBA", "User", "Night Vision"]);
        assert_eq!(p.options(by_id("gamevisual").unwrap())[5].value, 0x0A);
        assert_eq!(labels(&p, "hdr_mode"), ["Cinema HDR", "Gaming HDR", "Console HDR", "True Black 500", "Dolby Vision Bright", "Dolby Vision Dark", "Dolby Vision Gaming", "Dolby Vision Source only"]);
        assert_eq!(labels(&p, "gamma"), ["1.8", "2.0", "2.2", "2.4", "2.6"]);
        assert_eq!(labels(&p, "kvm"), ["USB upstream B", "USB upstream C", "Auto KVM"]);
        assert_eq!(labels(&p, "pip_mode").len(), 6);
        assert_eq!(labels(&p, "pip_source_a"), ["DisplayPort", "HDMI-1", "HDMI-2", "USB-C"]);
        assert_eq!(labels(&p, "crosshair").len(), 10);
        assert_eq!(labels(&p, "shortcut_1").len(), 12);
        assert_eq!(labels(&p, "color_temp"), ["4000K", "5000K", "6500K", "7500K", "8200K", "9300K", "10000K", "User"]);
    }

    #[test]
    fn mode_rules() {
        let b = |id| by_id(id).unwrap();
        let fps = Context { mode: 7, ..Default::default() };
        assert!(enabled(b("saturation"), &fps) && !enabled(b("blue_light"), &fps) && !enabled(b("red_gain"), &fps));
        let srgb = Context { mode: 0x0A, ..Default::default() };
        assert!(enabled(b("brightness"), &srgb) && !enabled(b("contrast"), &srgb));
        let racing_max = Context { mode: 5, blf_max: true, ..Default::default() };
        assert!(enabled(b("blue_light"), &racing_max) && !enabled(b("brightness"), &racing_max));
        let hdr = Context { mode: 5, hdr: true, ..Default::default() };
        assert!(enabled(b("hdr_mode"), &hdr) && !enabled(b("brightness"), &hdr) && enabled(b("screen_move"), &hdr));
        assert!(enabled(b("gamevisual"), &hdr));
        let ps = Context { power_saving: true, ..Default::default() };
        assert!(enabled(b("brightness"), &ps) && !enabled(b("crosshair"), &ps) && !enabled(b("screen_move"), &ps));
        // MOBA drops saturation, Shadow Boost and sharpness; Night Vision only saturation.
        let moba = Context { mode: 8, ..Default::default() };
        assert!(!enabled(b("saturation"), &moba) && !enabled(b("shadow_boost"), &moba) && !enabled(b("sharpness"), &moba));
        assert!(enabled(b("contrast"), &moba) && enabled(b("color_temp"), &moba));
        let night = Context { mode: 9, ..Default::default() };
        assert!(!enabled(b("saturation"), &night) && enabled(b("shadow_boost"), &night) && enabled(b("sharpness"), &night));
        // Hue is never usable on Gaming and the HDR picker only in HDR.
        for mode in 1..=10 {
            let cx = Context { mode, ..Default::default() };
            assert!(!enabled(b("hue"), &cx) && !enabled(b("hdr_mode"), &cx), "mode {mode}");
            assert!(enabled(b("gamevisual"), &cx) && enabled(b("reset"), &cx) && enabled(b("overdrive"), &cx), "mode {mode}");
        }
    }

    fn game_visual_ids() -> impl Iterator<Item = &'static str> {
        FEATURES.iter().filter(|f| f.page == Page::GameVisual).map(|f| f.id)
    }

    #[test]
    fn hdr_leaves_the_mode_row_and_colour_temperature() {
        let hdr = Context { mode: 7, hdr: true, ..Default::default() };
        let on: Vec<_> = game_visual_ids().filter(|id| enabled(by_id(id).unwrap(), &hdr)).collect();
        assert_eq!(on, ["gamevisual", "hdr_mode", "color_temp"]);
        // Everything off the GameVisual page stays as usable as in SDR.
        for f in FEATURES.iter().filter(|f| f.page != Page::GameVisual) {
            assert!(enabled(f, &hdr), "{}", f.id);
        }
        // Mode and user colour do not matter any more: HDR decides.
        let hdr_srgb_user = Context { mode: 0x0A, hdr: true, user_color: true, ..Default::default() };
        assert!(enabled(by_id("color_temp").unwrap(), &hdr_srgb_user) && !enabled(by_id("red_gain").unwrap(), &hdr_srgb_user));
    }

    #[test]
    fn power_saving_leaves_system_pages_brightness_and_contrast() {
        let ps = Context { mode: 7, power_saving: true, ..Default::default() };
        for f in FEATURES {
            let want = matches!(f.page, Page::System | Page::DeviceInfo | Page::MultiScreen) || matches!(f.id, "brightness" | "contrast");
            assert_eq!(enabled(f, &ps), want, "{}", f.id);
        }
        // It beats HDR and sRGB, which would each decide differently.
        let both = Context { mode: 0x0A, hdr: true, power_saving: true, ..Default::default() };
        assert!(enabled(by_id("brightness").unwrap(), &both) && enabled(by_id("contrast").unwrap(), &both));
        assert!(!enabled(by_id("hdr_mode").unwrap(), &both) && !enabled(by_id("gamevisual").unwrap(), &both));
        assert!(enabled(by_id("volume").unwrap(), &both) && !enabled(by_id("crosshair").unwrap(), &both));
    }

    #[test]
    fn srgb_is_either_value() {
        for mode in [0x03, 0x0A] {
            let cx = Context { mode, ..Default::default() };
            let off: Vec<_> = game_visual_ids().filter(|id| !enabled(by_id(id).unwrap(), &cx)).collect();
            assert_eq!(off, ["hdr_mode", "contrast", "blue_light", "shadow_boost", "ascr", "saturation", "hue", "color_temp", "red_gain", "green_gain", "blue_gain", "gamma"], "mode {mode}");
            // Brightness, overdrive, sharpness and the mode row stay.
            for id in ["brightness", "overdrive", "sharpness", "gamevisual", "reset"] {
                assert!(enabled(by_id(id).unwrap(), &cx), "{id} in mode {mode}");
            }
        }
        // sRGB wins over User colour temperature: the gains stay off.
        let srgb_user = Context { mode: 0x0A, user_color: true, ..Default::default() };
        assert!(!enabled(by_id("red_gain").unwrap(), &srgb_user));
    }

    #[test]
    fn rgb_gains_need_user_colour_temperature() {
        for id in ["red_gain", "green_gain", "blue_gain"] {
            let f = by_id(id).unwrap();
            assert!(!enabled(f, &Context { mode: 7, ..Default::default() }), "{id}");
            assert!(enabled(f, &Context { mode: 7, user_color: true, ..Default::default() }), "{id}");
            // The mode matters too: not in sRGB, but MOBA is fine.
            assert!(enabled(f, &Context { mode: 8, user_color: true, ..Default::default() }), "{id}");
        }
    }

    #[test]
    fn blue_light_only_in_racing() {
        for mode in 1..=10u8 {
            let cx = Context { mode, ..Default::default() };
            assert_eq!(enabled(by_id("blue_light").unwrap(), &cx), mode == 5, "mode {mode}");
        }
        // Brightness is locked by the strongest filter level, but only in Racing.
        let locked = |mode| enabled(by_id("brightness").unwrap(), &Context { mode, blf_max: true, ..Default::default() });
        assert!(!locked(5) && locked(7));
    }

    #[test]
    fn hdr_is_non_zero_and_not_0xfe() {
        assert!(!hdr_active(0) && !hdr_active(0xFE));
        assert!(hdr_active(0x0101) && hdr_active(0x0207) && hdr_active(1));
    }

    #[test]
    fn context_from_registers() {
        let reply = |code: u8, max: u16, current: u16| Some(VcpReply { code, kind: sumi_proto::VcpType::SetParameter, max, current });
        let regs = |c: u8| match c {
            0xDC => reply(c, 0x0A, 0x0105),
            0xE2 => reply(c, 0x0208, 0x0102),
            0xE1 => reply(c, 1, 1),
            0x14 => reply(c, 0x0B, 0x0B),
            0xE6 => reply(c, 4, 4),
            _ => None,
        };
        let cx = Context::from_regs(true, regs);
        assert_eq!(cx, Context { mode: 5, hdr: true, power_saving: true, user_color: true, blf_max: true });
        // 0xE1 is ASCR on the old scheme, not power saving.
        assert!(!Context::from_regs(false, regs).power_saving);
        // Unreadable registers read as "off".
        assert_eq!(Context::from_regs(true, |_| None), Context::default());
        // A maximum of 0 is "no such control", not "at the maximum".
        let flat = |c: u8| if c == 0xE6 { reply(c, 0, 0) } else { None };
        assert!(!Context::from_regs(true, flat).blf_max);
        // 0xFE in 0xE2 is no HDR.
        let fe = |c: u8| if c == 0xE2 { reply(c, 0x0208, 0xFE) } else { None };
        assert!(!Context::from_regs(true, fe).hdr);
    }

    #[test]
    fn only_the_pixel_cleaning_bit_is_a_trigger() {
        assert_eq!(trigger_bits(0xFD), 0x0010);
        assert_eq!(trigger_bits(0xFC), 0);
        assert_eq!(trigger_bits(0x10), 0);
        // No toggle shares a bit with it, and every trigger is on 0xFD.
        for f in FEATURES {
            if matches!(f.encoding, Bit { .. }) && f.code == 0xFD {
                assert_eq!(bit_mask(f) & trigger_bits(0xFD), 0, "{}", f.id);
            }
            if matches!(f.encoding, Trigger { .. }) {
                assert_eq!(f.code, 0xFD, "{}", f.id);
                assert_eq!(f.risk, Risk::PanelMaintenance);
            }
        }
    }

    #[test]
    fn bit_masks_do_not_overlap_within_a_register() {
        for (i, a) in FEATURES.iter().enumerate() {
            for b in &FEATURES[i + 1..] {
                if a.code == b.code && bit_mask(a) != 0 && bit_mask(b) != 0 {
                    assert_eq!(bit_mask(a) & bit_mask(b), 0, "{} and {}", a.id, b.id);
                }
            }
        }
    }

    #[test]
    fn options_are_filtered_by_the_capability_string() {
        let caps = sumi_proto::caps::parse(CAPS);
        let p = profile(&caps);
        // Every option offered is one the monitor listed, for the plain choice features.
        for (id, code) in [("timer", 0xE4), ("shadow_boost", 0xE5), ("pixel_cleaning_reminder", 0xF8), ("screen_move", 0xF9), ("language", 0xCC)] {
            let f = by_id(id).unwrap();
            for o in p.options(f) {
                assert!(p.has_lo(code, o.value), "{id} offers {}", o.label);
            }
        }
        // True Black 400 (0x0104) is not in this monitor's 0xE2 list.
        assert!(!p.options(by_id("hdr_mode").unwrap()).iter().any(|o| o.value == 0x0104));
        // The old-style crosshairs are filtered out of a new-style monitor.
        assert!(p.options(by_id("crosshair").unwrap()).iter().all(|o| o.value == 0 || o.value >= 7));
    }

    #[test]
    fn old_scheme_monitors_get_only_the_generic_controls() {
        let caps = sumi_proto::caps::parse(CAPS);
        let p = Profile { new_vcp: false, ..profile(&caps) };
        let ids: Vec<_> = FEATURES.iter().filter(|f| p.supported(f)).map(|f| f.id).collect();
        assert!(ids.contains(&"brightness") && ids.contains(&"firmware") && ids.contains(&"input"));
        assert!(!ids.contains(&"pixel_cleaning") && !ids.contains(&"hdr_mode") && !ids.contains(&"gamevisual"));
    }
}
