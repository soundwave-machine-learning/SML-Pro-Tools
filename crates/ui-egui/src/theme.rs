//! Design tokens: the one place colours, type sizes and control metrics are defined
//! (docs/STYLE_GUIDE.md). `Tokens::DARK` is "SML Studio Dark" — charcoal surfaces, a restrained
//! cyan/blue accent for focus and selection, green for playback, red for recording and
//! critical states, amber for warnings. Colours were chosen by eye; they are ours.

use egui::{Color32, FontFamily, FontId};
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the Light palette is active. Dark is the default (the incumbent is dark-only).
static LIGHT_THEME: AtomicBool = AtomicBool::new(false);

/// The active palette (R20: themes are validated data). Written by the UI thread when the
/// theme changes; every painter reads a copy per frame.
static CURRENT: RwLock<Tokens> = RwLock::new(Tokens::DARK);

/// Colours as `[r, g, b, a]` in theme files.
mod rgba {
    use egui::Color32;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(c: &Color32, s: S) -> Result<S::Ok, S::Error> {
        c.to_srgba_unmultiplied().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Color32, D::Error> {
        let [r, g, b, a] = <[u8; 4]>::deserialize(d)?;
        Ok(Color32::from_rgba_unmultiplied(r, g, b, a))
    }
}

/// Unit tests that change the process-wide palette must own it until their assertions finish.
#[cfg(test)]
pub(crate) fn lock_test_palette() -> std::sync::MutexGuard<'static, ()> {
    static PALETTE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    PALETTE.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The Appearance setting (`ui.theme {mode}`). Dark by default; System and Light are opt-in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    System,
    Light,
    #[default]
    Dark,
    /// Built-in high-contrast palette (black surfaces, white text, saturated states).
    #[serde(rename = "high_contrast")]
    HighContrast,
    /// The user's validated theme file (`UiState.theme_custom`); Dark when none is loaded.
    Custom,
    /// Built-in alternate dark palette: softer charcoal with a steel-teal accent, for long
    /// sessions in dim rooms. Same token semantics as Dark.
    Graphite,
}

impl ThemeMode {
    pub const ALL: [Self; 6] = [Self::Dark, Self::Graphite, Self::Light, Self::HighContrast, Self::System, Self::Custom];

    pub const fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::HighContrast => "High contrast",
            Self::Custom => "Custom",
            Self::Graphite => "Graphite",
        }
    }

    /// The id used in `ui.theme {"mode": …}` and in the saved UI prefs.
    pub const fn id(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
            Self::HighContrast => "high_contrast",
            Self::Custom => "custom",
            Self::Graphite => "graphite",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.id().eq_ignore_ascii_case(s.trim()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Tokens {
    #[serde(with = "rgba")]
    pub window_bg: Color32,
    #[serde(with = "rgba")]
    pub panel_bg: Color32,
    #[serde(with = "rgba")]
    pub panel_bg2: Color32,
    #[serde(with = "rgba")]
    pub toolbar_bg: Color32,
    #[serde(with = "rgba")]
    pub toolbar_group: Color32,
    #[serde(with = "rgba")]
    pub border: Color32,
    #[serde(with = "rgba")]
    pub border_light: Color32,
    #[serde(with = "rgba")]
    pub text: Color32,
    #[serde(with = "rgba")]
    pub text_dim: Color32,
    #[serde(with = "rgba")]
    pub text_dark: Color32,
    #[serde(with = "rgba")]
    pub header_text: Color32,
    #[serde(with = "rgba")]
    pub counter_bg: Color32,
    #[serde(with = "rgba")]
    pub counter_text: Color32,
    #[serde(with = "rgba")]
    pub counter_label: Color32,
    #[serde(with = "rgba")]
    pub accent: Color32,
    #[serde(with = "rgba")]
    pub accent_dark: Color32,
    #[serde(with = "rgba")]
    pub mode_on: Color32,
    #[serde(with = "rgba")]
    pub mode_on_text: Color32,
    #[serde(with = "rgba")]
    pub mode_off_text: Color32,
    #[serde(with = "rgba")]
    pub button: Color32,
    #[serde(with = "rgba")]
    pub button_hi: Color32,
    #[serde(with = "rgba")]
    pub button_border: Color32,
    #[serde(with = "rgba")]
    pub name_field: Color32,
    #[serde(with = "rgba")]
    pub name_field_sel: Color32,
    #[serde(with = "rgba")]
    pub ruler_bg: Color32,
    #[serde(with = "rgba")]
    pub ruler_label_bg: Color32,
    #[serde(with = "rgba")]
    pub ruler_text: Color32,
    #[serde(with = "rgba")]
    pub ruler_tick: Color32,
    #[serde(with = "rgba")]
    pub tempo_ruler: Color32,
    #[serde(with = "rgba")]
    pub meter_ruler: Color32,
    #[serde(with = "rgba")]
    pub marker_ruler: Color32,
    #[serde(with = "rgba")]
    pub playlist_bg: Color32,
    #[serde(with = "rgba")]
    pub playlist_alt: Color32,
    #[serde(with = "rgba")]
    pub grid_line: Color32,
    #[serde(with = "rgba")]
    pub bar_line: Color32,
    #[serde(with = "rgba")]
    pub selection: Color32,
    #[serde(with = "rgba")]
    pub playhead: Color32,
    #[serde(with = "rgba")]
    pub insertion: Color32,
    #[serde(with = "rgba")]
    pub solo: Color32,
    #[serde(with = "rgba")]
    pub mute: Color32,
    #[serde(with = "rgba")]
    pub rec: Color32,
    #[serde(with = "rgba")]
    pub input: Color32,
    #[serde(with = "rgba")]
    pub auto_read: Color32,
    #[serde(with = "rgba")]
    pub auto_write: Color32,
    #[serde(with = "rgba")]
    pub auto_touch: Color32,
    #[serde(with = "rgba")]
    pub auto_latch: Color32,
    #[serde(with = "rgba")]
    pub meter_green: Color32,
    #[serde(with = "rgba")]
    pub meter_yellow: Color32,
    #[serde(with = "rgba")]
    pub meter_red: Color32,
    #[serde(with = "rgba")]
    pub fader_track: Color32,
    #[serde(with = "rgba")]
    pub strip_bg: Color32,
    #[serde(with = "rgba")]
    pub strip_section: Color32,
    #[serde(with = "rgba")]
    pub slot_bg: Color32,
    #[serde(with = "rgba")]
    pub automation_line: Color32,
    /// Menu bar background.
    #[serde(with = "rgba")]
    pub menu_bar: Color32,
    /// Outline of toolbar groups.
    #[serde(with = "rgba")]
    pub group_border: Color32,
    /// Edit-mode cell, not selected.
    #[serde(with = "rgba")]
    pub mode_off: Color32,
    /// Counter-panel toggle (Grid, Count Off), off.
    #[serde(with = "rgba")]
    pub counter_off: Color32,
    /// Outline of the counter panels.
    #[serde(with = "rgba")]
    pub counter_border: Color32,
    /// Divider inside the counter panels.
    #[serde(with = "rgba")]
    pub counter_rule: Color32,
    /// Smart-tool bracket, off.
    #[serde(with = "rgba")]
    pub tool_bracket_off: Color32,
    /// Selected row in the Tracks, Groups and Clips lists.
    #[serde(with = "rgba")]
    pub row_selected: Color32,
    /// Track-visible dot in the Tracks list.
    #[serde(with = "rgba")]
    pub visible_dot: Color32,
    /// Big Counter background.
    #[serde(with = "rgba")]
    pub big_counter_bg: Color32,
    /// Edit-window track header, selected.
    #[serde(with = "rgba")]
    pub header_selected: Color32,
    /// Mix-window strip, selected.
    #[serde(with = "rgba")]
    pub strip_selected: Color32,
    /// Marker names on the marker ruler.
    #[serde(with = "rgba")]
    pub marker_text: Color32,
    /// Alternate-playlist lanes.
    #[serde(with = "rgba")]
    pub playlist_lane: Color32,
    /// Universe overview background.
    #[serde(with = "rgba")]
    pub universe_bg: Color32,
    /// Universe visible-area outline.
    #[serde(with = "rgba")]
    pub universe_view: Color32,
    /// Meter background.
    #[serde(with = "rgba")]
    pub meter_bg: Color32,
    /// Meter clip indicator, not clipped.
    #[serde(with = "rgba")]
    pub meter_clip_off: Color32,
    /// Pan knob body.
    #[serde(with = "rgba")]
    pub knob_bg: Color32,
    /// Pan knob outline.
    #[serde(with = "rgba")]
    pub knob_ring: Color32,
    /// Pan knob pointer.
    #[serde(with = "rgba")]
    pub knob_pointer: Color32,
    /// Mix insert slot holding a plugin.
    #[serde(with = "rgba")]
    pub insert_on: Color32,
    /// Mix insert slot, bypassed.
    #[serde(with = "rgba")]
    pub insert_bypass: Color32,
    /// Mix send slot in use.
    #[serde(with = "rgba")]
    pub send_on: Color32,
    /// Mix insert/send slot outline.
    #[serde(with = "rgba")]
    pub slot_border: Color32,
    /// Mix-window section captions (INSERTS A-E, I/O…).
    #[serde(with = "rgba")]
    pub section_label: Color32,
    /// Raised surfaces: the control bar, popovers' groups, cards inside panels.
    #[serde(with = "rgba")]
    pub surface_raised: Color32,
    /// Keyboard focus ring and the focused/active outline (restrained cyan).
    #[serde(with = "rgba")]
    pub focus: Color32,
    /// Playback running (Play button lit, transport state).
    #[serde(with = "rgba")]
    pub playback: Color32,
    /// Warnings: unavailable or unverified states, mismatches that do not block.
    #[serde(with = "rgba")]
    pub warning: Color32,
    /// Errors and faults (failed plugin, missing device); not the Record colour.
    #[serde(with = "rgba")]
    pub error: Color32,
    /// Text of disabled controls and unavailable values.
    #[serde(with = "rgba")]
    pub text_disabled: Color32,
    /// Outline of selected ranges, rows and strips (with `selection` fill).
    #[serde(with = "rgba")]
    pub selection_stroke: Color32,
    /// The loop range (Loop lane and loop markers).
    #[serde(with = "rgba")]
    pub loop_range: Color32,
}

impl Tokens {
    pub const DARK: Tokens = Tokens {
        window_bg: Color32::from_rgb(24, 25, 27),
        panel_bg: Color32::from_rgb(31, 32, 35),
        panel_bg2: Color32::from_rgb(38, 40, 43),
        toolbar_bg: Color32::from_rgb(27, 28, 31),
        toolbar_group: Color32::from_rgb(21, 22, 24),
        border: Color32::from_rgb(11, 12, 13),
        border_light: Color32::from_rgb(58, 61, 66),
        text: Color32::from_rgb(224, 226, 230),
        text_dim: Color32::from_rgb(158, 163, 171),
        text_dark: Color32::from_rgb(16, 16, 16),
        header_text: Color32::from_rgb(204, 208, 214),
        counter_bg: Color32::from_rgb(4, 4, 4),
        counter_text: Color32::from_rgb(104, 220, 120),
        counter_label: Color32::from_rgb(200, 200, 200),
        accent: Color32::from_rgb(44, 132, 192),
        accent_dark: Color32::from_rgb(30, 88, 130),
        mode_on: Color32::from_rgb(92, 196, 108),
        mode_on_text: Color32::from_rgb(10, 30, 12),
        mode_off_text: Color32::from_rgb(98, 196, 112),
        button: Color32::from_rgb(52, 55, 59),
        button_hi: Color32::from_rgb(66, 70, 76),
        button_border: Color32::from_rgb(16, 17, 18),
        name_field: Color32::from_rgb(208, 208, 208),
        name_field_sel: Color32::from_rgb(232, 232, 232),
        ruler_bg: Color32::from_rgb(35, 37, 40),
        ruler_label_bg: Color32::from_rgb(35, 37, 40),
        ruler_text: Color32::from_rgb(186, 190, 197),
        ruler_tick: Color32::from_rgb(92, 96, 103),
        tempo_ruler: Color32::from_rgb(52, 128, 88),
        meter_ruler: Color32::from_rgb(44, 108, 140),
        marker_ruler: Color32::from_rgb(41, 43, 46),
        playlist_bg: Color32::from_rgb(30, 31, 34),
        playlist_alt: Color32::from_rgb(34, 35, 38),
        grid_line: Color32::from_rgb(46, 48, 52),
        bar_line: Color32::from_rgb(64, 67, 73),
        selection: Color32::from_rgba_premultiplied(30, 70, 104, 112),
        playhead: Color32::from_rgb(240, 72, 60),
        insertion: Color32::from_rgb(230, 230, 230),
        solo: Color32::from_rgb(222, 196, 52),
        mute: Color32::from_rgb(232, 148, 40),
        rec: Color32::from_rgb(220, 58, 52),
        input: Color32::from_rgb(70, 170, 90),
        auto_read: Color32::from_rgb(96, 200, 110),
        auto_write: Color32::from_rgb(220, 70, 60),
        auto_touch: Color32::from_rgb(230, 170, 50),
        auto_latch: Color32::from_rgb(160, 120, 220),
        meter_green: Color32::from_rgb(60, 200, 80),
        meter_yellow: Color32::from_rgb(230, 210, 60),
        meter_red: Color32::from_rgb(230, 50, 40),
        fader_track: Color32::from_rgb(12, 12, 12),
        strip_bg: Color32::from_rgb(39, 41, 44),
        strip_section: Color32::from_rgb(30, 31, 34),
        slot_bg: Color32::from_rgb(23, 24, 26),
        automation_line: Color32::from_rgb(240, 240, 240),
        menu_bar: Color32::from_rgb(19, 20, 22),
        group_border: Color32::from_rgb(9, 10, 11),
        mode_off: Color32::from_rgb(20, 36, 22),
        counter_off: Color32::from_rgb(30, 50, 32),
        counter_border: Color32::from_rgb(50, 50, 50),
        counter_rule: Color32::from_rgb(40, 40, 40),
        tool_bracket_off: Color32::from_rgb(64, 64, 66),
        row_selected: Color32::from_rgb(34, 70, 102),
        visible_dot: Color32::from_rgb(200, 200, 200),
        big_counter_bg: Color32::from_rgb(0, 0, 0),
        header_selected: Color32::from_rgb(38, 56, 74),
        strip_selected: Color32::from_rgb(40, 58, 78),
        marker_text: Color32::from_rgb(236, 236, 236),
        playlist_lane: Color32::from_rgb(26, 27, 29),
        universe_bg: Color32::from_rgb(17, 18, 20),
        universe_view: Color32::from_rgb(255, 255, 255),
        meter_bg: Color32::from_rgb(8, 8, 8),
        meter_clip_off: Color32::from_rgb(60, 20, 20),
        knob_bg: Color32::from_rgb(24, 25, 28),
        knob_ring: Color32::from_rgb(88, 92, 100),
        knob_pointer: Color32::from_rgb(230, 230, 230),
        insert_on: Color32::from_rgb(42, 66, 92),
        insert_bypass: Color32::from_rgb(70, 56, 30),
        send_on: Color32::from_rgb(38, 70, 58),
        slot_border: Color32::from_rgb(16, 16, 16),
        section_label: Color32::from_rgb(170, 176, 186),
        surface_raised: Color32::from_rgb(36, 38, 42),
        focus: Color32::from_rgb(88, 196, 232),
        playback: Color32::from_rgb(70, 196, 112),
        warning: Color32::from_rgb(236, 168, 56),
        error: Color32::from_rgb(240, 92, 80),
        text_disabled: Color32::from_rgb(104, 108, 116),
        selection_stroke: Color32::from_rgb(86, 170, 226),
        loop_range: Color32::from_rgb(232, 150, 52),
    };

    pub const LIGHT: Tokens = Tokens {
        window_bg: Color32::from_rgb(238, 239, 242),
        panel_bg: Color32::from_rgb(248, 249, 251),
        panel_bg2: Color32::from_rgb(255, 255, 255),
        toolbar_bg: Color32::from_rgb(230, 232, 236),
        toolbar_group: Color32::from_rgb(242, 243, 246),
        border: Color32::from_rgb(190, 193, 200),
        border_light: Color32::from_rgb(210, 213, 220),
        text: Color32::from_rgb(35, 36, 40),
        text_dim: Color32::from_rgb(95, 98, 106),
        text_dark: Color32::from_rgb(20, 21, 24),
        header_text: Color32::from_rgb(45, 47, 53),
        counter_bg: Color32::from_rgb(224, 226, 231),
        counter_text: Color32::from_rgb(22, 116, 45),
        counter_label: Color32::from_rgb(65, 68, 75),
        accent: Color32::from_rgb(62, 116, 180),
        accent_dark: Color32::from_rgb(43, 88, 145),
        mode_on: Color32::from_rgb(75, 158, 88),
        mode_on_text: Color32::from_rgb(255, 255, 255),
        mode_off_text: Color32::from_rgb(48, 122, 62),
        button: Color32::from_rgb(224, 226, 230),
        button_hi: Color32::from_rgb(210, 213, 219),
        button_border: Color32::from_rgb(175, 179, 187),
        name_field: Color32::from_rgb(255, 255, 255),
        name_field_sel: Color32::from_rgb(236, 242, 252),
        ruler_bg: Color32::from_rgb(226, 228, 233),
        ruler_label_bg: Color32::from_rgb(226, 228, 233),
        ruler_text: Color32::from_rgb(70, 73, 80),
        ruler_tick: Color32::from_rgb(130, 134, 142),
        tempo_ruler: Color32::from_rgb(104, 170, 124),
        meter_ruler: Color32::from_rgb(92, 158, 184),
        marker_ruler: Color32::from_rgb(210, 213, 219),
        playlist_bg: Color32::from_rgb(246, 247, 249),
        playlist_alt: Color32::from_rgb(238, 240, 244),
        grid_line: Color32::from_rgb(210, 213, 219),
        bar_line: Color32::from_rgb(185, 189, 197),
        selection: Color32::from_rgba_premultiplied(100, 145, 205, 90),
        playhead: Color32::from_rgb(210, 48, 42),
        insertion: Color32::from_rgb(35, 36, 40),
        solo: Color32::from_rgb(178, 145, 20),
        mute: Color32::from_rgb(194, 106, 22),
        rec: Color32::from_rgb(190, 42, 38),
        input: Color32::from_rgb(45, 126, 64),
        auto_read: Color32::from_rgb(52, 142, 68),
        auto_write: Color32::from_rgb(190, 50, 45),
        auto_touch: Color32::from_rgb(180, 122, 24),
        auto_latch: Color32::from_rgb(118, 80, 170),
        meter_green: Color32::from_rgb(36, 160, 58),
        meter_yellow: Color32::from_rgb(190, 162, 20),
        meter_red: Color32::from_rgb(195, 42, 35),
        fader_track: Color32::from_rgb(198, 201, 207),
        strip_bg: Color32::from_rgb(235, 237, 241),
        strip_section: Color32::from_rgb(244, 245, 247),
        slot_bg: Color32::from_rgb(248, 249, 251),
        automation_line: Color32::from_rgb(40, 42, 48),
        menu_bar: Color32::from_rgb(222, 224, 229),
        group_border: Color32::from_rgb(190, 193, 200),
        mode_off: Color32::from_rgb(236, 240, 237),
        counter_off: Color32::from_rgb(214, 230, 217),
        counter_border: Color32::from_rgb(190, 193, 200),
        counter_rule: Color32::from_rgb(204, 207, 213),
        tool_bracket_off: Color32::from_rgb(200, 203, 210),
        row_selected: Color32::from_rgb(198, 216, 240),
        visible_dot: Color32::from_rgb(110, 113, 120),
        big_counter_bg: Color32::from_rgb(224, 226, 231),
        header_selected: Color32::from_rgb(206, 219, 238),
        strip_selected: Color32::from_rgb(206, 219, 238),
        marker_text: Color32::from_rgb(35, 36, 40),
        playlist_lane: Color32::from_rgb(232, 234, 238),
        universe_bg: Color32::from_rgb(228, 230, 234),
        universe_view: Color32::from_rgb(40, 42, 48),
        meter_bg: Color32::from_rgb(196, 199, 206),
        meter_clip_off: Color32::from_rgb(232, 200, 198),
        knob_bg: Color32::from_rgb(252, 252, 253),
        knob_ring: Color32::from_rgb(150, 154, 162),
        knob_pointer: Color32::from_rgb(40, 42, 48),
        insert_on: Color32::from_rgb(206, 218, 236),
        insert_bypass: Color32::from_rgb(240, 222, 186),
        send_on: Color32::from_rgb(204, 230, 214),
        slot_border: Color32::from_rgb(175, 179, 187),
        section_label: Color32::from_rgb(95, 98, 106),
        surface_raised: Color32::from_rgb(255, 255, 255),
        focus: Color32::from_rgb(20, 120, 190),
        playback: Color32::from_rgb(30, 140, 62),
        warning: Color32::from_rgb(176, 110, 10),
        error: Color32::from_rgb(190, 40, 36),
        text_disabled: Color32::from_rgb(150, 154, 162),
        selection_stroke: Color32::from_rgb(52, 110, 180),
        loop_range: Color32::from_rgb(196, 112, 20),
    };

    /// The active palette.
    pub fn current() -> Tokens {
        *CURRENT.read().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Built-in high-contrast starting theme: black surfaces, white text, saturated state
    /// colours, thick borders. Dark-based (egui dark visuals).
    pub const HIGH_CONTRAST: Tokens = Tokens {
        window_bg: Color32::BLACK,
        panel_bg: Color32::BLACK,
        panel_bg2: Color32::from_rgb(16, 16, 16),
        toolbar_bg: Color32::BLACK,
        toolbar_group: Color32::from_rgb(12, 12, 12),
        border: Color32::WHITE,
        border_light: Color32::from_rgb(200, 200, 200),
        text: Color32::WHITE,
        text_dim: Color32::from_rgb(220, 220, 220),
        text_dark: Color32::BLACK,
        header_text: Color32::WHITE,
        counter_bg: Color32::BLACK,
        counter_text: Color32::from_rgb(0, 255, 120),
        counter_label: Color32::WHITE,
        accent: Color32::from_rgb(255, 220, 0),
        accent_dark: Color32::from_rgb(200, 170, 0),
        mode_on: Color32::from_rgb(0, 255, 120),
        mode_on_text: Color32::BLACK,
        mode_off_text: Color32::from_rgb(0, 255, 120),
        button: Color32::from_rgb(28, 28, 28),
        button_hi: Color32::from_rgb(64, 64, 64),
        button_border: Color32::WHITE,
        name_field: Color32::WHITE,
        name_field_sel: Color32::from_rgb(255, 255, 160),
        ruler_bg: Color32::BLACK,
        ruler_label_bg: Color32::BLACK,
        ruler_text: Color32::WHITE,
        ruler_tick: Color32::from_rgb(200, 200, 200),
        tempo_ruler: Color32::from_rgb(0, 160, 90),
        meter_ruler: Color32::from_rgb(0, 120, 200),
        marker_ruler: Color32::from_rgb(20, 20, 20),
        playlist_bg: Color32::BLACK,
        playlist_alt: Color32::from_rgb(14, 14, 14),
        grid_line: Color32::from_rgb(90, 90, 90),
        bar_line: Color32::from_rgb(160, 160, 160),
        selection: Color32::from_rgba_premultiplied(90, 120, 200, 140),
        playhead: Color32::from_rgb(255, 60, 60),
        insertion: Color32::WHITE,
        solo: Color32::from_rgb(255, 230, 0),
        mute: Color32::from_rgb(255, 140, 0),
        rec: Color32::from_rgb(255, 40, 40),
        input: Color32::from_rgb(0, 220, 90),
        auto_read: Color32::from_rgb(0, 230, 110),
        auto_write: Color32::from_rgb(255, 60, 50),
        auto_touch: Color32::from_rgb(255, 190, 0),
        auto_latch: Color32::from_rgb(0, 190, 255),
        meter_green: Color32::from_rgb(0, 230, 90),
        meter_yellow: Color32::from_rgb(255, 230, 0),
        meter_red: Color32::from_rgb(255, 40, 40),
        fader_track: Color32::from_rgb(60, 60, 60),
        strip_bg: Color32::BLACK,
        strip_section: Color32::from_rgb(14, 14, 14),
        slot_bg: Color32::from_rgb(10, 10, 10),
        automation_line: Color32::from_rgb(0, 255, 160),
        menu_bar: Color32::BLACK,
        group_border: Color32::WHITE,
        mode_off: Color32::from_rgb(20, 20, 20),
        counter_off: Color32::from_rgb(20, 20, 20),
        counter_border: Color32::WHITE,
        counter_rule: Color32::from_rgb(120, 120, 120),
        tool_bracket_off: Color32::from_rgb(120, 120, 120),
        row_selected: Color32::from_rgb(60, 60, 0),
        visible_dot: Color32::from_rgb(0, 255, 120),
        big_counter_bg: Color32::BLACK,
        header_selected: Color32::from_rgb(50, 50, 0),
        strip_selected: Color32::from_rgb(50, 50, 0),
        marker_text: Color32::WHITE,
        playlist_lane: Color32::from_rgb(10, 10, 10),
        universe_bg: Color32::BLACK,
        universe_view: Color32::from_rgb(255, 220, 0),
        meter_bg: Color32::BLACK,
        meter_clip_off: Color32::from_rgb(40, 40, 40),
        knob_bg: Color32::from_rgb(20, 20, 20),
        knob_ring: Color32::WHITE,
        knob_pointer: Color32::from_rgb(255, 220, 0),
        insert_on: Color32::from_rgb(0, 120, 200),
        insert_bypass: Color32::from_rgb(90, 90, 90),
        send_on: Color32::from_rgb(0, 150, 90),
        slot_border: Color32::WHITE,
        section_label: Color32::WHITE,
        surface_raised: Color32::from_rgb(20, 20, 20),
        focus: Color32::from_rgb(0, 220, 255),
        playback: Color32::from_rgb(0, 255, 120),
        warning: Color32::from_rgb(255, 190, 0),
        error: Color32::from_rgb(255, 60, 60),
        text_disabled: Color32::from_rgb(170, 170, 170),
        selection_stroke: Color32::from_rgb(255, 220, 0),
        loop_range: Color32::from_rgb(255, 170, 0),
    };

    /// "Graphite": the alternate dark palette. Lifted, lower-contrast surfaces and a steel-teal
    /// accent; state colours (record, playback, warnings, meters) are Dark's, so their meaning
    /// never changes between palettes.
    pub const GRAPHITE: Tokens = Tokens {
        window_bg: Color32::from_rgb(34, 35, 37),
        panel_bg: Color32::from_rgb(42, 43, 46),
        panel_bg2: Color32::from_rgb(50, 52, 55),
        toolbar_bg: Color32::from_rgb(38, 39, 42),
        toolbar_group: Color32::from_rgb(30, 31, 33),
        menu_bar: Color32::from_rgb(28, 29, 31),
        border: Color32::from_rgb(18, 19, 20),
        border_light: Color32::from_rgb(74, 77, 82),
        button: Color32::from_rgb(62, 65, 70),
        button_hi: Color32::from_rgb(78, 82, 88),
        button_border: Color32::from_rgb(22, 23, 25),
        strip_bg: Color32::from_rgb(48, 50, 53),
        strip_section: Color32::from_rgb(40, 41, 44),
        slot_bg: Color32::from_rgb(31, 32, 34),
        surface_raised: Color32::from_rgb(47, 49, 52),
        playlist_bg: Color32::from_rgb(39, 40, 43),
        playlist_alt: Color32::from_rgb(43, 44, 47),
        playlist_lane: Color32::from_rgb(35, 36, 38),
        ruler_bg: Color32::from_rgb(44, 46, 49),
        ruler_label_bg: Color32::from_rgb(44, 46, 49),
        marker_ruler: Color32::from_rgb(50, 52, 55),
        grid_line: Color32::from_rgb(55, 57, 61),
        bar_line: Color32::from_rgb(72, 75, 81),
        accent: Color32::from_rgb(52, 138, 148),
        accent_dark: Color32::from_rgb(36, 96, 104),
        focus: Color32::from_rgb(112, 202, 208),
        selection: Color32::from_rgba_premultiplied(36, 84, 90, 112),
        selection_stroke: Color32::from_rgb(100, 190, 196),
        row_selected: Color32::from_rgb(44, 82, 88),
        header_selected: Color32::from_rgb(50, 66, 72),
        strip_selected: Color32::from_rgb(52, 70, 76),
        insert_on: Color32::from_rgb(46, 74, 82),
        text: Color32::from_rgb(220, 222, 224),
        text_dim: Color32::from_rgb(164, 168, 174),
        section_label: Color32::from_rgb(178, 182, 188),
        ..Tokens::DARK
    };

    /// Every token by name, for the theme editor.
    pub fn fields_mut(&mut self) -> Vec<(&'static str, &mut Color32)> {
        vec![
            ("window_bg", &mut self.window_bg),
            ("panel_bg", &mut self.panel_bg),
            ("panel_bg2", &mut self.panel_bg2),
            ("toolbar_bg", &mut self.toolbar_bg),
            ("toolbar_group", &mut self.toolbar_group),
            ("border", &mut self.border),
            ("border_light", &mut self.border_light),
            ("text", &mut self.text),
            ("text_dim", &mut self.text_dim),
            ("text_dark", &mut self.text_dark),
            ("header_text", &mut self.header_text),
            ("counter_bg", &mut self.counter_bg),
            ("counter_text", &mut self.counter_text),
            ("counter_label", &mut self.counter_label),
            ("accent", &mut self.accent),
            ("accent_dark", &mut self.accent_dark),
            ("mode_on", &mut self.mode_on),
            ("mode_on_text", &mut self.mode_on_text),
            ("mode_off_text", &mut self.mode_off_text),
            ("button", &mut self.button),
            ("button_hi", &mut self.button_hi),
            ("button_border", &mut self.button_border),
            ("name_field", &mut self.name_field),
            ("name_field_sel", &mut self.name_field_sel),
            ("ruler_bg", &mut self.ruler_bg),
            ("ruler_label_bg", &mut self.ruler_label_bg),
            ("ruler_text", &mut self.ruler_text),
            ("ruler_tick", &mut self.ruler_tick),
            ("tempo_ruler", &mut self.tempo_ruler),
            ("meter_ruler", &mut self.meter_ruler),
            ("marker_ruler", &mut self.marker_ruler),
            ("playlist_bg", &mut self.playlist_bg),
            ("playlist_alt", &mut self.playlist_alt),
            ("grid_line", &mut self.grid_line),
            ("bar_line", &mut self.bar_line),
            ("selection", &mut self.selection),
            ("playhead", &mut self.playhead),
            ("insertion", &mut self.insertion),
            ("solo", &mut self.solo),
            ("mute", &mut self.mute),
            ("rec", &mut self.rec),
            ("input", &mut self.input),
            ("auto_read", &mut self.auto_read),
            ("auto_write", &mut self.auto_write),
            ("auto_touch", &mut self.auto_touch),
            ("auto_latch", &mut self.auto_latch),
            ("meter_green", &mut self.meter_green),
            ("meter_yellow", &mut self.meter_yellow),
            ("meter_red", &mut self.meter_red),
            ("fader_track", &mut self.fader_track),
            ("strip_bg", &mut self.strip_bg),
            ("strip_section", &mut self.strip_section),
            ("slot_bg", &mut self.slot_bg),
            ("automation_line", &mut self.automation_line),
            ("menu_bar", &mut self.menu_bar),
            ("group_border", &mut self.group_border),
            ("mode_off", &mut self.mode_off),
            ("counter_off", &mut self.counter_off),
            ("counter_border", &mut self.counter_border),
            ("counter_rule", &mut self.counter_rule),
            ("tool_bracket_off", &mut self.tool_bracket_off),
            ("row_selected", &mut self.row_selected),
            ("visible_dot", &mut self.visible_dot),
            ("big_counter_bg", &mut self.big_counter_bg),
            ("header_selected", &mut self.header_selected),
            ("strip_selected", &mut self.strip_selected),
            ("marker_text", &mut self.marker_text),
            ("playlist_lane", &mut self.playlist_lane),
            ("universe_bg", &mut self.universe_bg),
            ("universe_view", &mut self.universe_view),
            ("meter_bg", &mut self.meter_bg),
            ("meter_clip_off", &mut self.meter_clip_off),
            ("knob_bg", &mut self.knob_bg),
            ("knob_ring", &mut self.knob_ring),
            ("knob_pointer", &mut self.knob_pointer),
            ("insert_on", &mut self.insert_on),
            ("insert_bypass", &mut self.insert_bypass),
            ("send_on", &mut self.send_on),
            ("slot_border", &mut self.slot_border),
            ("section_label", &mut self.section_label),
            ("surface_raised", &mut self.surface_raised),
            ("focus", &mut self.focus),
            ("playback", &mut self.playback),
            ("warning", &mut self.warning),
            ("error", &mut self.error),
            ("text_disabled", &mut self.text_disabled),
            ("selection_stroke", &mut self.selection_stroke),
            ("loop_range", &mut self.loop_range),
        ]
    }
}

impl Default for Tokens {
    fn default() -> Self {
        Tokens::DARK
    }
}

/// A theme file (R20): validated data only — semantic colour tokens, a base (dark/light
/// visuals), a typography scale within bounds. No scripts, no resources, nothing that touches
/// plugin editors.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThemeData {
    pub format: String,
    #[serde(default)]
    pub name: String,
    /// `dark`, `light` or `high_contrast`: which built-in the tokens extend.
    #[serde(default)]
    pub base: ThemeMode,
    pub tokens: Tokens,
    #[serde(default = "one")]
    pub typography_scale: f32,
}

fn one() -> f32 {
    1.0
}

pub const THEME_FORMAT: &str = "sml-theme-1";
pub const TYPOGRAPHY_SCALE: (f32, f32) = (0.8, 1.5);
/// Minimum text-to-surface contrast ratio a theme must keep (WCAG large-text level).
pub const MIN_CONTRAST: f32 = 3.0;

fn luminance(c: Color32) -> f32 {
    let ch = |v: u8| {
        let x = f32::from(v) / 255.0;
        if x <= 0.039_28 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
    };
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)
}

/// WCAG contrast ratio.
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Validate a theme: known format, a built-in base, typography within bounds, readable text on
/// every surface, state colours distinct from their resting surface. The first failure is the
/// reason shown to the user; an invalid theme is never applied.
pub fn validate(d: &ThemeData) -> Result<(), String> {
    if d.format != THEME_FORMAT {
        return Err(format!("unsupported theme format `{}` (expected {THEME_FORMAT})", d.format));
    }
    if matches!(d.base, ThemeMode::Custom | ThemeMode::System) {
        return Err("`base` must be dark, light or high_contrast".into());
    }
    if !d.typography_scale.is_finite() || d.typography_scale < TYPOGRAPHY_SCALE.0 || d.typography_scale > TYPOGRAPHY_SCALE.1 {
        return Err(format!("typography_scale {} is outside {}..{}", d.typography_scale, TYPOGRAPHY_SCALE.0, TYPOGRAPHY_SCALE.1));
    }
    let t = &d.tokens;
    for (what, fg, bg) in [
        ("text on panels", t.text, t.panel_bg),
        ("text on the window", t.text, t.window_bg),
        ("counter digits", t.counter_text, t.counter_bg),
        ("ruler text", t.ruler_text, t.ruler_bg),
        ("button text", t.text, t.button),
        ("selected-mode text", t.mode_on_text, t.mode_on),
    ] {
        let c = contrast(fg, bg);
        if c < MIN_CONTRAST {
            return Err(format!("{what}: contrast {c:.2}:1 is below {MIN_CONTRAST}:1"));
        }
    }
    for (what, state, rest) in
        [("record", t.rec, t.button), ("solo", t.solo, t.button), ("mute", t.mute, t.button), ("meter clip", t.meter_red, t.meter_clip_off)]
    {
        if contrast(state, rest) < 1.5 {
            return Err(format!("{what} state is not distinguishable from its resting colour"));
        }
    }
    Ok(())
}

/// Parse and validate a theme file.
pub fn import(text: &str) -> Result<ThemeData, String> {
    let d: ThemeData = serde_json::from_str(text).map_err(|e| format!("not a theme file: {e}"))?;
    validate(&d)?;
    Ok(d)
}

/// The current palette as a theme file (for export and as the editor's starting point).
pub fn export(name: &str, base: ThemeMode, typography_scale: f32) -> ThemeData {
    ThemeData { format: THEME_FORMAT.into(), name: name.into(), base, tokens: Tokens::current(), typography_scale }
}

/// Prefs loader for `theme_custom`: a broken or invalid saved theme becomes `None` (the app
/// launches in Dark and says so) instead of failing the whole preferences file.
pub fn lenient_custom<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<ThemeData>, D::Error> {
    let v: Option<serde_json::Value> = serde::Deserialize::deserialize(d)?;
    Ok(v.and_then(|v| serde_json::from_value::<ThemeData>(v).ok()).filter(|t| validate(t).is_ok()))
}

/// Resolve the mode to (light visuals?, palette). Custom without a valid theme falls back to Dark.
pub fn resolve(ctx: &egui::Context, mode: ThemeMode, custom: Option<&ThemeData>) -> (bool, Tokens) {
    match mode {
        ThemeMode::Light => (true, Tokens::LIGHT),
        ThemeMode::Dark => (false, Tokens::DARK),
        ThemeMode::HighContrast => (false, Tokens::HIGH_CONTRAST),
        ThemeMode::Graphite => (false, Tokens::GRAPHITE),
        ThemeMode::System => {
            if ctx.system_theme() == Some(egui::Theme::Light) {
                (true, Tokens::LIGHT)
            } else {
                (false, Tokens::DARK)
            }
        }
        ThemeMode::Custom => match custom.filter(|t| validate(t).is_ok()) {
            Some(t) => (t.base == ThemeMode::Light, t.tokens),
            None => (false, Tokens::DARK),
        },
    }
}

/// Whether the Light palette is the active one.
pub fn is_light() -> bool {
    LIGHT_THEME.load(Ordering::Relaxed)
}

/// Resolve each frame from the integration's current OS appearance. Unknown uses Dark.
pub fn wants_light(ctx: &egui::Context, mode: ThemeMode) -> bool {
    match mode {
        ThemeMode::Light => true,
        ThemeMode::Dark | ThemeMode::HighContrast | ThemeMode::Custom | ThemeMode::Graphite => false,
        ThemeMode::System => ctx.system_theme() == Some(egui::Theme::Light),
    }
}

/// Track colours tinted for clip bodies.
pub fn clip_colors(rgb: [u8; 3], selected: bool) -> (Color32, Color32, Color32) {
    let [r, g, b] = rgb;
    let k = if selected { 0.95 } else { 0.62 };
    let body = Color32::from_rgb((f32::from(r) * k) as u8, (f32::from(g) * k) as u8, (f32::from(b) * k) as u8);
    let bar = Color32::from_rgb((f32::from(r) * 0.9) as u8, (f32::from(g) * 0.9) as u8, (f32::from(b) * 0.9) as u8);
    if is_light() {
        // A paler body and a waveform in a deep shade of the clip's own colour.
        let tint = |c: u8, k: f32| (f32::from(c) + (255.0 - f32::from(c)) * k) as u8;
        let k = if selected { 0.2 } else { 0.45 };
        let body = Color32::from_rgb(tint(r, k), tint(g, k), tint(b, k));
        let d = if selected { 0.22 } else { 0.32 };
        let wave = Color32::from_rgb((f32::from(r) * d) as u8, (f32::from(g) * d) as u8, (f32::from(b) * d) as u8);
        return (body, bar, wave);
    }
    let wave = if selected { Color32::from_rgb(20, 20, 24) } else { Color32::from_rgb(12, 12, 14) };
    (body, bar, wave)
}

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// Type scale in logical points (docs/STYLE_GUIDE.md). Use these, not ad-hoc sizes.
pub mod size {
    /// Section captions (INSERTS, I / O), units beside values. The smallest text in the app.
    pub const CAPTION: f32 = 10.0;
    /// Secondary text, hints, status details.
    pub const SMALL: f32 = 11.0;
    /// Labels and controls.
    pub const BODY: f32 = 13.0;
    /// Panel and window titles.
    pub const TITLE: f32 = 14.0;
    /// Numeric readouts (dB, BPM, positions) — always monospaced so digits line up.
    pub const NUMERIC: f32 = 13.0;
    /// Primary numeric entry in the control bar (BPM).
    pub const NUMERIC_LARGE: f32 = 15.0;
}

/// Control metrics in logical points (docs/STYLE_GUIDE.md).
pub mod metric {
    /// Standard control height (buttons, combo boxes, text fields in bars and dialogs).
    pub const CONTROL_H: f32 = 22.0;
    /// Compact control height (mixer strip rows, lists).
    pub const CONTROL_H_SMALL: f32 = 18.0;
    /// Corner radius of controls and groups.
    pub const RADIUS: u8 = 3;
    /// Space between controls in a group.
    pub const GAP: f32 = 6.0;
    /// Space between groups.
    pub const GROUP_GAP: f32 = 12.0;
}

/// A group frame on a bar or panel: raised surface, hairline border, standard radius.
pub fn group_frame() -> egui::Frame {
    let t = Tokens::current();
    egui::Frame::NONE
        .fill(t.surface_raised)
        .stroke(egui::Stroke::new(1.0, t.border))
        .corner_radius(egui::CornerRadius::same(metric::RADIUS))
        .inner_margin(egui::Margin::symmetric(6, 2))
}

/// Make the built-in `light` (or dark) palette active and apply its visuals.
pub fn apply(ctx: &egui::Context, light: bool) {
    apply_tokens(ctx, light, if light { Tokens::LIGHT } else { Tokens::DARK });
}

/// Install `t` as the active palette with `light` (or dark) egui visuals. Call only when the
/// palette changes.
pub fn apply_tokens(ctx: &egui::Context, light: bool, t: Tokens) {
    LIGHT_THEME.store(light, Ordering::Relaxed);
    *CURRENT.write().unwrap_or_else(std::sync::PoisonError::into_inner) = t;
    let mut v = if light { egui::Visuals::light() } else { egui::Visuals::dark() };
    v.panel_fill = t.panel_bg;
    v.window_fill = t.panel_bg2;
    v.extreme_bg_color = t.slot_bg;
    v.faint_bg_color = t.panel_bg2;
    v.selection.bg_fill = t.accent;
    v.widgets.inactive.weak_bg_fill = t.button;
    v.widgets.inactive.bg_fill = t.button;
    v.widgets.hovered.weak_bg_fill = t.button_hi;
    v.widgets.noninteractive.fg_stroke.color = t.text;
    v.widgets.inactive.fg_stroke.color = t.text;
    // Control states (docs/STYLE_GUIDE.md): resting = button fill with a dark hairline;
    // hovered = lighter fill and a visible edge; pressed = accent fill with the focus edge;
    // selected = accent fill. Disabled controls fade (disabled_alpha), they never vanish.
    let radius = egui::CornerRadius::same(metric::RADIUS);
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = radius;
    }
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, t.border_light);
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, t.button_border);
    v.widgets.hovered.bg_fill = t.button_hi;
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, t.border_light);
    v.widgets.hovered.fg_stroke.color = t.text;
    v.widgets.active.bg_fill = t.accent_dark;
    v.widgets.active.weak_bg_fill = t.accent_dark;
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0, t.focus);
    v.widgets.active.fg_stroke.color = t.text;
    v.widgets.open.weak_bg_fill = t.button_hi;
    v.widgets.open.bg_stroke = egui::Stroke::new(1.0, t.focus);
    v.selection.stroke = egui::Stroke::new(1.0, t.text);
    v.text_cursor.stroke = egui::Stroke::new(2.0, t.focus);
    v.hyperlink_color = t.focus;
    v.warn_fg_color = t.warning;
    v.error_fg_color = t.error;
    v.window_stroke = egui::Stroke::new(1.0, t.border_light);
    v.disabled_alpha = 0.45;
    v.window_corner_radius = egui::CornerRadius::same(6);
    v.menu_corner_radius = egui::CornerRadius::same(4);
    // Keep receiving native OS appearance changes even when our palette is manual. On macOS,
    // pinning egui's theme also pins the window appearance and hides later OS changes.
    ctx.set_theme(egui::ThemePreference::System);
    // Both egui style branches must use our resolved palette: egui can select either branch
    // as the OS changes, including while SoundCraft's manual Light/Dark choice stays fixed.
    ctx.all_styles_mut(|s| {
        s.visuals = v.clone();
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(6.0, 2.0);
        s.interaction.tooltip_delay = 0.4;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Services, SoundApp, UiState};
    use serde_json::json;

    /// R20: built-ins validate; a theme file round-trips; invalid files (format, scale,
    /// unreadable text, indistinguishable states, garbage) are rejected with a reason and never
    /// applied; a broken saved theme falls back to Dark at launch.
    #[test]
    fn themes_are_validated_data_with_fallback() {
        for (base, t) in [
            (ThemeMode::Dark, Tokens::DARK),
            (ThemeMode::Light, Tokens::LIGHT),
            (ThemeMode::HighContrast, Tokens::HIGH_CONTRAST),
            (ThemeMode::Graphite, Tokens::GRAPHITE),
        ] {
            validate(&ThemeData { format: THEME_FORMAT.into(), name: base.label().into(), base, tokens: t, typography_scale: 1.0 }).unwrap();
        }
        assert!(contrast(Tokens::HIGH_CONTRAST.text, Tokens::HIGH_CONTRAST.panel_bg) > 15.0);
        let d = ThemeData { format: THEME_FORMAT.into(), name: "mine".into(), base: ThemeMode::Dark, tokens: Tokens::DARK, typography_scale: 1.2 };
        let text = serde_json::to_string_pretty(&d).unwrap();
        assert!(text.contains("\"window_bg\": [\n"), "colours are [r,g,b,a] arrays: {text}");
        assert_eq!(import(&text).unwrap(), d);
        let bad_format = text.replace(THEME_FORMAT, "sml-theme-9");
        assert!(import(&bad_format).unwrap_err().contains("format"));
        let bad_scale = text.replace("\"typography_scale\": 1.2", "\"typography_scale\": 3.0");
        assert!(import(&bad_scale).unwrap_err().contains("typography_scale"));
        let mut low = d.clone();
        low.tokens.text = low.tokens.panel_bg;
        let err = validate(&low).unwrap_err();
        assert!(err.contains("contrast"), "{err}");
        let mut same = d.clone();
        same.tokens.rec = same.tokens.button;
        assert!(validate(&same).unwrap_err().contains("record"));
        assert!(import("{not json").is_err());
        assert!(
            import("{\"format\":\"sml-theme-1\",\"tokens\":{\"text\":[0,0,0,255]}}").is_err(),
            "missing tokens default to Dark, black text on dark fails contrast"
        );
        // Partial token files are allowed: unspecified tokens come from Dark.
        let partial = import("{\"format\":\"sml-theme-1\",\"tokens\":{\"accent\":[255,0,255,255]}}").unwrap();
        assert_eq!(partial.tokens.accent, Color32::from_rgb(255, 0, 255));
        assert_eq!(partial.tokens.text, Tokens::DARK.text);
        // A broken saved theme never bricks launch: prefs load, the mode resolves to Dark.
        let prefs: UiState =
            serde_json::from_value(json!({"theme": "custom", "theme_custom": {"format": "sml-theme-1", "tokens": {"text": [0, 0, 0, 255]}}}))
                .unwrap();
        assert_eq!(prefs.theme, ThemeMode::Custom);
        assert!(prefs.theme_custom.is_none());
        let ctx = egui::Context::default();
        assert_eq!(resolve(&ctx, ThemeMode::Custom, prefs.theme_custom.as_ref()), (false, Tokens::DARK));
        assert_eq!(resolve(&ctx, ThemeMode::HighContrast, None), (false, Tokens::HIGH_CONTRAST));
        assert_eq!(resolve(&ctx, ThemeMode::Custom, Some(&d)).1, Tokens::DARK);
        let mut editable = Tokens::DARK;
        assert_eq!(editable.fields_mut().len(), 88);
        assert_eq!(ThemeMode::parse("high_contrast"), Some(ThemeMode::HighContrast));
    }

    #[test]
    fn dark_is_the_default() {
        assert_eq!(ThemeMode::default(), ThemeMode::Dark);
        assert_eq!(UiState::default().theme, ThemeMode::Dark);
        // Prefs saved before the setting existed load as Dark.
        let old: UiState = serde_json::from_value(json!({"show_tracks_list": true})).unwrap_or_default();
        assert_eq!(old.theme, ThemeMode::Dark);
    }

    #[test]
    fn modes_round_trip_through_prefs() {
        for mode in ThemeMode::ALL {
            assert_eq!(ThemeMode::parse(mode.id()), Some(mode));
            let ui = UiState { theme: mode, ..UiState::default() };
            let back: UiState = serde_json::from_value(serde_json::to_value(&ui).unwrap_or_default()).unwrap_or_default();
            assert_eq!(back.theme, mode);
        }
        assert_eq!(ThemeMode::parse(" Light "), Some(ThemeMode::Light));
        assert_eq!(ThemeMode::parse("sepia"), None);
    }

    #[test]
    fn theme_command_sets_and_reports_the_mode() {
        let mut app = SoundApp::new(soundcraft_engine::Engine::default(), None, Services::default());
        assert_eq!(app.run("ui.theme", json!({})), Ok(json!({"mode": "dark"})));
        assert_eq!(app.run("ui.theme", json!({"mode": "light"})), Ok(json!({"mode": "light"})));
        assert_eq!(app.ui.theme, ThemeMode::Light);
        assert!(app.run("ui.theme", json!({"mode": "sepia"})).is_err());
        assert_eq!(app.ui.theme, ThemeMode::Light);
        assert_eq!(app.run("ui.theme", json!({"mode": "system"})), Ok(json!({"mode": "system"})));
        assert!(crate::menus::UI_COMMANDS.iter().any(|c| c.0 == "ui.theme"));
    }

    #[test]
    fn application_frames_follow_system_appearance_and_preserve_manual_palettes() {
        // Mixer rendering tests also change the process-wide custom-painted palette.
        let _palette = lock_test_palette();
        fn frame(app: &mut SoundApp, ctx: &egui::Context, system_theme: Option<egui::Theme>, light: bool) {
            let mode = app.ui.theme;
            let colors = |app: &SoundApp| {
                app.engine
                    .session()
                    .tracks
                    .iter()
                    .map(|track| {
                        (track.color, track.playlists.iter().flat_map(|playlist| &playlist.clips).map(|clip| clip.color).collect::<Vec<_>>())
                    })
                    .collect::<Vec<_>>()
            };
            let before = colors(app);
            let mut output = ctx.run_ui(egui::RawInput { system_theme, ..Default::default() }, |ui| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                let tokens = if light { Tokens::LIGHT } else { Tokens::DARK };
                assert_eq!(is_light(), light, "custom palette for {mode:?} with {system_theme:?}");
                assert_eq!(Tokens::current().panel_bg, tokens.panel_bg);
                assert_eq!(ctx.options(|options| options.theme_preference), egui::ThemePreference::System);
                for theme in [egui::Theme::Light, egui::Theme::Dark] {
                    let style = ctx.style_of(theme);
                    assert_eq!(style.visuals.dark_mode, !light);
                    assert_eq!(style.visuals.panel_fill, tokens.panel_bg);
                    assert_eq!(style.visuals.widgets.inactive.bg_fill, tokens.button);
                    assert_eq!(style.visuals.widgets.inactive.fg_stroke.color, tokens.text);
                    assert_eq!(style.spacing.button_padding, egui::vec2(6.0, 2.0));
                }
                app.ui(ui);
                assert_eq!(ui.visuals().panel_fill, tokens.panel_bg);
                assert_eq!(ui.visuals().widgets.inactive.bg_fill, tokens.button);
            });
            output.textures_delta.clear();
            assert_eq!(app.ui.theme, mode);
            assert_eq!(serde_json::to_value(&app.ui).unwrap()["theme"], mode.id());
            assert_eq!(colors(app), before, "appearance must preserve track and clip colors");
        }

        let appearances = [Some(egui::Theme::Light), Some(egui::Theme::Dark), Some(egui::Theme::Light), None];
        for initial in [Some(egui::Theme::Light), Some(egui::Theme::Dark), None] {
            let ctx = egui::Context::default();
            let mut app = SoundApp::new(soundcraft_engine::Engine::default(), None, Services::default());
            app.ui = serde_json::from_value(json!({"theme": "system"})).unwrap();
            frame(&mut app, &ctx, initial, initial == Some(egui::Theme::Light));
        }

        let ctx = egui::Context::default();
        let mut engine = soundcraft_engine::demo::demo_engine();
        let track = engine.session_mut().tracks.first_mut().unwrap();
        track.color = [19, 83, 147];
        track.playlists.iter_mut().flat_map(|playlist| &mut playlist.clips).next().unwrap().color = Some([211, 57, 99]);
        let mut app = SoundApp::new(engine, None, Services::default());
        for mode in [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark] {
            app.run("ui.theme", json!({"mode": mode.id()})).unwrap();
            for appearance in appearances {
                let light = mode == ThemeMode::Light || (mode == ThemeMode::System && appearance == Some(egui::Theme::Light));
                frame(&mut app, &ctx, appearance, light);
            }
            if mode != ThemeMode::System {
                let latest = if mode == ThemeMode::Light { egui::Theme::Dark } else { egui::Theme::Light };
                frame(&mut app, &ctx, Some(latest), mode == ThemeMode::Light);
                app.run("ui.theme", json!({"mode": "system"})).unwrap();
                // Resolve the current context immediately, without another OS appearance event.
                app.logic(&ctx);
                assert_eq!(is_light(), latest == egui::Theme::Light);
                assert_eq!(ctx.global_style().visuals.dark_mode, latest != egui::Theme::Light);
                frame(&mut app, &ctx, Some(latest), latest == egui::Theme::Light);
            }
        }
    }
}
