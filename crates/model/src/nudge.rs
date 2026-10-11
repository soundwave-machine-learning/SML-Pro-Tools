//! Persistent state of the dedicated Nudge panel (NUDGE_SPEC): value, unit, target and the exact
//! logical anchor used to accumulate fractional steps without drift. Saved with the session, so
//! undo/redo and save/reopen restore it together with the edited coordinates.

/// Unit of the nudge value. Grid and Nudge are separate settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum NudgeUnit {
    Samples,
    Milliseconds,
    Seconds,
    /// Musical ticks (960 per quarter note), mapped through the tempo map at the anchor.
    Ticks,
    /// Timecode frames at the session frame rate.
    Frames,
}

impl NudgeUnit {
    pub fn label(self) -> &'static str {
        match self {
            NudgeUnit::Samples => "samples",
            NudgeUnit::Milliseconds => "ms",
            NudgeUnit::Seconds => "s",
            NudgeUnit::Ticks => "ticks",
            NudgeUnit::Frames => "frames",
        }
    }
    pub fn parse(s: &str) -> Option<NudgeUnit> {
        Some(match s {
            "samples" | "sample" | "smp" => NudgeUnit::Samples,
            "ms" | "milliseconds" => NudgeUnit::Milliseconds,
            "s" | "seconds" | "sec" => NudgeUnit::Seconds,
            "ticks" | "tick" | "bars_beats" => NudgeUnit::Ticks,
            "frames" | "frame" | "timecode" => NudgeUnit::Frames,
            _ => return None,
        })
    }
}

/// What a nudge moves. Command ids are distinct per target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum NudgeTarget {
    /// Selected clips (rigid set incl. clip groups); the cursor/selection when none is selected.
    Clips,
    /// Slide audio inside fixed clip boundaries.
    ClipContents,
    TrimStart,
    TrimEnd,
    /// Move the edit selection/cursor only (no audio changes).
    Selection,
    SelectionStart,
    SelectionEnd,
}

impl NudgeTarget {
    pub const ALL: [NudgeTarget; 7] = [
        NudgeTarget::Clips,
        NudgeTarget::ClipContents,
        NudgeTarget::TrimStart,
        NudgeTarget::TrimEnd,
        NudgeTarget::Selection,
        NudgeTarget::SelectionStart,
        NudgeTarget::SelectionEnd,
    ];
    pub fn id(self) -> &'static str {
        match self {
            NudgeTarget::Clips => "clips",
            NudgeTarget::ClipContents => "contents",
            NudgeTarget::TrimStart => "trim_start",
            NudgeTarget::TrimEnd => "trim_end",
            NudgeTarget::Selection => "selection",
            NudgeTarget::SelectionStart => "selection_start",
            NudgeTarget::SelectionEnd => "selection_end",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            NudgeTarget::Clips => "Selected clips",
            NudgeTarget::ClipContents => "Clip contents",
            NudgeTarget::TrimStart => "Left trim edge",
            NudgeTarget::TrimEnd => "Right trim edge",
            NudgeTarget::Selection => "Selection/cursor only",
            NudgeTarget::SelectionStart => "Selection start",
            NudgeTarget::SelectionEnd => "Selection end",
        }
    }
    pub fn parse(s: &str) -> Option<NudgeTarget> {
        NudgeTarget::ALL.into_iter().find(|t| t.id() == s)
    }
}

/// Exact accumulation anchor for one target/unit/value: the logical position the first step
/// started from, the signed number of steps taken since, and the integer displacement actually
/// applied so far. Positions are recomputed from the anchor (rounded once, ties-to-even), so
/// reversing a sequence returns exactly to the anchor.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NudgeAnchor {
    pub target: NudgeTarget,
    pub unit: NudgeUnit,
    /// Bits of the f64 value (exact identity check).
    pub value_bits: u64,
    /// Sorted ids moved (clip ids, or empty for selection targets).
    pub ids: Vec<u64>,
    /// Logical anchor sample (group anchor: earliest selected clip start / selection point).
    pub anchor: i64,
    pub steps: i64,
    /// Integer displacement applied so far, relative to `anchor`.
    pub applied: i64,
    /// Sample rate + tempo-map fingerprint the anchor was computed under.
    pub context: u64,
}

/// The Nudge panel: value/unit/target plus the accumulation anchor.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NudgePanel {
    pub value: f64,
    pub unit: NudgeUnit,
    pub target: NudgeTarget,
    /// Clip-gain nudge step in dB (separate from time nudge).
    pub gain_step_db: f64,
    #[serde(default)]
    pub anchor: Option<NudgeAnchor>,
}

impl Default for NudgePanel {
    /// SML default for vocals: 1 ms, selected clips.
    fn default() -> Self {
        NudgePanel { value: 1.0, unit: NudgeUnit::Milliseconds, target: NudgeTarget::Clips, gain_step_db: 1.0, anchor: None }
    }
}

/// SML presets (not a transcription of another product's menu).
pub const NUDGE_PRESETS: &[(f64, NudgeUnit)] = &[
    (1.0, NudgeUnit::Samples),
    (10.0, NudgeUnit::Samples),
    (100.0, NudgeUnit::Samples),
    (0.1, NudgeUnit::Milliseconds),
    (1.0, NudgeUnit::Milliseconds),
    (5.0, NudgeUnit::Milliseconds),
    (10.0, NudgeUnit::Milliseconds),
    (20.0, NudgeUnit::Milliseconds),
    (50.0, NudgeUnit::Milliseconds),
    (100.0, NudgeUnit::Milliseconds),
    (1.0, NudgeUnit::Seconds),
    (240.0, NudgeUnit::Ticks),
    (320.0, NudgeUnit::Ticks),
    (360.0, NudgeUnit::Ticks),
    (480.0, NudgeUnit::Ticks),
    (960.0, NudgeUnit::Ticks),
    (1.0, NudgeUnit::Frames),
];
