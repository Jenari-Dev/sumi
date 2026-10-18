//! Hot-key actions Sumi registers with KDE global shortcuts (labels are
//! English msgids; sumid and the app translate them through the `sumi` catalogue).

/// (action id, label) in DisplayWidget's HotKey page order, then Sumi extras.
pub const ACTIONS: &[(&str, &str)] = &[
    ("osd_show", "EZ OSD: open the monitor menu"),
    ("osd_up", "EZ OSD: up"),
    ("osd_down", "EZ OSD: down"),
    ("osd_left", "EZ OSD: left"),
    ("osd_right", "EZ OSD: right"),
    ("osd_enter", "EZ OSD: enter"),
    ("osd_back", "EZ OSD: back"),
    ("osd_close", "EZ OSD: close the monitor menu"),
    ("brightness_up", "Brightness +"),
    ("brightness_down", "Brightness −"),
    ("contrast_up", "Contrast +"),
    ("contrast_down", "Contrast −"),
    ("blue_light_up", "Blue Light Filter +"),
    ("blue_light_down", "Blue Light Filter −"),
    ("shadow_boost_up", "Shadow Boost +"),
    ("shadow_boost_down", "Shadow Boost −"),
    ("saturation_up", "Saturation +"),
    ("saturation_down", "Saturation −"),
    ("pip_toggle", "PIP/PBP on/off"),
    ("kvm_toggle", "KVM: switch USB upstream"),
    ("power_saving_toggle", "Power saving on/off"),
    ("gamevisual_next", "Next GameVisual mode"),
    ("gamevisual_prev", "Previous GameVisual mode"),
    ("target_mode_toggle", "Target Mode on/off"),
    ("hdr_toggle", "HDR on/off"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_labels_are_unique_and_snake_case() {
        let mut ids: Vec<_> = ACTIONS.iter().map(|(id, _)| *id).collect();
        let mut labels: Vec<_> = ACTIONS.iter().map(|(_, l)| *l).collect();
        ids.sort();
        labels.sort();
        ids.dedup();
        labels.dedup();
        assert_eq!((ids.len(), labels.len()), (ACTIONS.len(), ACTIONS.len()));
        assert!(ids.iter().all(|id| id.chars().all(|c| c.is_ascii_lowercase() || c == '_')));
    }

    #[test]
    fn every_label_is_offered_for_translation() {
        let all = crate::strings::all();
        for (id, label) in ACTIONS {
            assert!(all.contains(label), "{id}: {label:?} is missing from strings::all()");
        }
    }
}

