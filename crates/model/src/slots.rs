//! Dedicated Summing / Tape slots and the SML mix profile (MIXER_SLOT_SPEC SL01–SL05,
//! MIXER_MODES_SPEC M01–M02).
//!
//! A slot is a fixed graph position that can host a provider (today: an installed external
//! VST3/CLAP plugin acting as a bus/track processor). An empty slot is the Clean (summing) or
//! None (tape) identity: exact pass-through, zero latency. Native summing/tape *models* are
//! deferred by the owner; nothing here implies a sound model exists.

use crate::Insert;

/// Which dedicated slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SlotKind {
    /// Audio tracks: after utility trim/polarity, before user Pre inserts.
    Tape,
    /// Buses and Main: on the summed input, before utility trim.
    Summing,
    /// Buses and Main: Pre (before user inserts) or Post (after fader/Post inserts).
    MasterTape,
}

impl SlotKind {
    pub fn id(self) -> &'static str {
        match self {
            SlotKind::Tape => "tape",
            SlotKind::Summing => "summing",
            SlotKind::MasterTape => "master_tape",
        }
    }
    pub fn parse(s: &str) -> Option<SlotKind> {
        Some(match s {
            "tape" => SlotKind::Tape,
            "summing" => SlotKind::Summing,
            "master_tape" => SlotKind::MasterTape,
            _ => return None,
        })
    }
    /// Instance slot index used for plugin instances (inserts use 0..n).
    pub fn instance_index(self) -> usize {
        match self {
            SlotKind::Tape => 1000,
            SlotKind::Summing => 1001,
            SlotKind::MasterTape => 1002,
        }
    }
    pub const ALL: [SlotKind; 3] = [SlotKind::Tape, SlotKind::Summing, SlotKind::MasterTape];
}

/// Master Tape placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum TapePosition {
    /// Before user Pre inserts (Main default).
    Pre,
    /// After fader and user Post inserts (bus default).
    #[default]
    Post,
}

/// One dedicated slot's saved state.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DedicatedSlot {
    /// Stable slot id (survives provider replacement, moves and reload).
    pub id: u64,
    /// Hosted provider; `None` = Clean/None identity.
    pub provider: Option<Insert>,
    /// Slot bypass: provider *and* its trim are bypassed together; compensation delay is kept.
    pub bypass: bool,
    /// Post-provider output trim (dB). Non-unity trim is visible processing even when empty.
    pub trim_db: f32,
}

impl DedicatedSlot {
    pub fn new(id: u64) -> DedicatedSlot {
        DedicatedSlot { id, provider: None, bypass: false, trim_db: 0.0 }
    }
    /// True when this slot changes nothing (empty and unity, or bypassed).
    pub fn is_identity(&self) -> bool {
        self.bypass || (self.provider.is_none() && self.trim_db == 0.0)
    }
}

/// The dedicated slots of one channel.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct ChannelSlots {
    #[serde(default)]
    pub tape: Option<DedicatedSlot>,
    #[serde(default)]
    pub summing: Option<DedicatedSlot>,
    #[serde(default)]
    pub master_tape: Option<DedicatedSlot>,
    #[serde(default)]
    pub master_tape_position: TapePosition,
    /// Tape group membership (schema only: groups may link mapped settings later, never audio).
    #[serde(default)]
    pub tape_group: Option<u64>,
}

impl ChannelSlots {
    pub fn get(&self, k: SlotKind) -> Option<&DedicatedSlot> {
        match k {
            SlotKind::Tape => self.tape.as_ref(),
            SlotKind::Summing => self.summing.as_ref(),
            SlotKind::MasterTape => self.master_tape.as_ref(),
        }
    }
    pub fn get_mut(&mut self, k: SlotKind) -> &mut Option<DedicatedSlot> {
        match k {
            SlotKind::Tape => &mut self.tape,
            SlotKind::Summing => &mut self.summing,
            SlotKind::MasterTape => &mut self.master_tape,
        }
    }
}

/// Temporary audition bypass (SL05). Not saved: reopen resets to normal; exports ignore it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct AuditionBypass {
    pub summing: bool,
    pub tape: bool,
    pub inserts: bool,
}

impl AuditionBypass {
    pub fn any(self) -> bool {
        self.summing || self.tape || self.inserts
    }
}

/// Mix profile (M01/M02). Standard keeps the legacy panner; Logic-style uses the SML profile laws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ProfileKind {
    #[default]
    Standard,
    LogicStyle,
}

/// How a stereo channel's panner behaves under the Logic-style profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum StereoPanMode {
    /// Balance: attenuates the opposite side, no crossfeed (uses `pan[0]`).
    #[default]
    Balance,
    /// Dual stereo pan: each input side through its own mono panner (`pan[0]`, `pan[1]`).
    StereoPan,
}

/// Session mixer graph/arithmetic version and profile. Version 1 = legacy (pre-SML) behavior;
/// version 2 = SL02 graph with slots, width and f64 bus sums. Sessions without the field load as
/// version 1 and keep their legacy sound until explicitly migrated.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MixProfile {
    #[serde(default = "legacy_version")]
    pub graph_version: u32,
    #[serde(default)]
    pub kind: ProfileKind,
    /// Pan law id (`0db`, `-3db`, `-3db_compensated`).
    #[serde(default = "default_law")]
    pub law: String,
    #[serde(default)]
    pub stereo_mode: StereoPanMode,
    /// Stereo-balancer compensation (shown explicitly, initially off).
    #[serde(default)]
    pub balance_compensated: bool,
}

fn legacy_version() -> u32 {
    1
}

fn default_law() -> String {
    "-3db".into()
}

impl Default for MixProfile {
    fn default() -> Self {
        MixProfile {
            graph_version: 1,
            kind: ProfileKind::Standard,
            law: default_law(),
            stereo_mode: StereoPanMode::Balance,
            balance_compensated: false,
        }
    }
}

/// Current graph version written by new SML sessions.
pub const GRAPH_VERSION: u32 = 2;

/// One channel in an A/B/C mix snapshot.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SnapshotChannel {
    pub track: crate::TrackId,
    pub volume_db: f32,
    pub pan: Vec<f32>,
    pub width: f32,
    pub mute: bool,
    pub solo: bool,
    pub hidden: bool,
}

/// A saved A/B/C mix snapshot (scope: gain, pan, width, mute, solo, visibility).
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct MixSnapshot {
    pub channels: Vec<SnapshotChannel>,
}
