//! Nudge keyboard dispatch (NUDGE_SPEC "Pro Tools-style Windows preset", N07 unit level).
//!
//! A pure mapping from (key, modifiers, focus) to a distinct nudge command, so focus rules and
//! the modifier matrix are testable headless. Known backend limit (recorded blocker): egui-winit
//! 0.36 maps `NumpadAdd`/`NumpadSubtract` to the same `Key::Plus`/`Key::Minus` as the main
//! keyboard row, so the UI cannot yet tell a keypad key from a text-row key. Until the backend
//! exposes physical keypad codes, plain Plus/Minus are treated as `Num+`/`Num-` *only* when no
//! text field, plugin editor, modal dialog or shortcut-capture has focus; physical keypad
//! verification (N07) remains a native gate.

/// Where keyboard focus is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// Edit window, no text entry: nudge keys are live.
    Edit,
    /// Explicit Edit Keyboard Focus (enables the laptop comma/period mapping).
    EditKeyboardFocus,
    TextField,
    PluginEditor,
    ModalDialog,
    ShortcutCapture,
}

/// A key as far as nudge cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NudgeKey {
    /// Keypad plus/minus (or a Plus/Minus whose source the backend cannot distinguish).
    NumPlus,
    NumMinus,
    Comma,
    Period,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// Windows/Start (super) key.
    pub win: bool,
}

/// What a nudge key does: a command id and a direction, or a change of the nudge value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NudgeAction {
    Command(&'static str, i64),
    /// Ctrl+Alt+Num± : next/previous nudge value.
    ChangeValue(i64),
}

/// The SML reference preset. Returns `None` when the key must not nudge (focus elsewhere, or an
/// unassigned chord such as Shift+Num±, which this preset deliberately leaves free).
pub fn binding(key: NudgeKey, m: Mods, focus: Focus) -> Option<NudgeAction> {
    match focus {
        Focus::TextField | Focus::PluginEditor | Focus::ModalDialog | Focus::ShortcutCapture => return None,
        Focus::Edit | Focus::EditKeyboardFocus => {}
    }
    let dir = match key {
        NudgeKey::NumPlus => 1,
        NudgeKey::NumMinus => -1,
        // Laptop alternative only under explicit Edit Keyboard Focus, and only plain.
        NudgeKey::Comma | NudgeKey::Period => {
            if focus != Focus::EditKeyboardFocus || m != Mods::default() {
                return None;
            }
            return Some(NudgeAction::Command("nudge.clips", if key == NudgeKey::Period { 1 } else { -1 }));
        }
    };
    let id = match (m.ctrl, m.alt, m.shift, m.win) {
        (false, false, false, false) => "nudge.clips",
        (false, false, false, true) => "nudge.contents",
        (false, true, false, false) => "nudge.trim_start",
        (true, false, false, false) => "nudge.trim_end",
        (false, true, true, false) => "nudge.selection_start",
        (true, false, true, false) => "nudge.selection_end",
        (true, true, false, false) => return Some(NudgeAction::ChangeValue(dir)),
        // Shift-only: reserved/unassigned in the reference preset (compatibility gap, see spec).
        _ => return None,
    };
    Some(NudgeAction::Command(id, dir))
}

#[cfg(test)]
mod nudge_keys_tests {
    use super::*;

    const NONE: Mods = Mods { ctrl: false, alt: false, shift: false, win: false };

    #[test]
    fn nudge_keys_modifier_matrix() {
        let c = |m: Mods| binding(NudgeKey::NumPlus, m, Focus::Edit);
        assert_eq!(c(NONE), Some(NudgeAction::Command("nudge.clips", 1)));
        assert_eq!(binding(NudgeKey::NumMinus, NONE, Focus::Edit), Some(NudgeAction::Command("nudge.clips", -1)));
        assert_eq!(c(Mods { win: true, ..NONE }), Some(NudgeAction::Command("nudge.contents", 1)));
        assert_eq!(c(Mods { alt: true, ..NONE }), Some(NudgeAction::Command("nudge.trim_start", 1)));
        assert_eq!(c(Mods { ctrl: true, ..NONE }), Some(NudgeAction::Command("nudge.trim_end", 1)));
        assert_eq!(c(Mods { alt: true, shift: true, ..NONE }), Some(NudgeAction::Command("nudge.selection_start", 1)));
        assert_eq!(c(Mods { ctrl: true, shift: true, ..NONE }), Some(NudgeAction::Command("nudge.selection_end", 1)));
        assert_eq!(c(Mods { ctrl: true, alt: true, ..NONE }), Some(NudgeAction::ChangeValue(1)));
        assert_eq!(c(Mods { shift: true, ..NONE }), None, "Shift-only stays unassigned");
    }

    #[test]
    fn nudge_keys_focus_blocks_edits() {
        for f in [Focus::TextField, Focus::PluginEditor, Focus::ModalDialog, Focus::ShortcutCapture] {
            assert_eq!(binding(NudgeKey::NumPlus, NONE, f), None, "{f:?}");
            assert_eq!(binding(NudgeKey::Period, NONE, f), None);
        }
    }

    #[test]
    fn nudge_keys_laptop_mapping_needs_explicit_focus() {
        assert_eq!(binding(NudgeKey::Period, NONE, Focus::Edit), None);
        assert_eq!(binding(NudgeKey::Period, NONE, Focus::EditKeyboardFocus), Some(NudgeAction::Command("nudge.clips", 1)));
        assert_eq!(binding(NudgeKey::Comma, NONE, Focus::EditKeyboardFocus), Some(NudgeAction::Command("nudge.clips", -1)));
        assert_eq!(binding(NudgeKey::Comma, Mods { ctrl: true, ..NONE }, Focus::EditKeyboardFocus), None);
    }

    #[test]
    fn nudge_keys_every_target_has_a_distinct_command() {
        let mut ids = Vec::new();
        for m in [
            NONE,
            Mods { win: true, ..NONE },
            Mods { alt: true, ..NONE },
            Mods { ctrl: true, ..NONE },
            Mods { alt: true, shift: true, ..NONE },
            Mods { ctrl: true, shift: true, ..NONE },
        ] {
            if let Some(NudgeAction::Command(id, _)) = binding(NudgeKey::NumPlus, m, Focus::Edit) {
                ids.push(id);
            }
        }
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n);
        assert_eq!(n, 6);
    }
}
