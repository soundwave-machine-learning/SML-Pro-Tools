//! SoundCraft's egui user interface.
//!
//! The UI is a thin shell over [`soundcraft_engine::Engine`]: it draws engine state and acts by
//! executing commands. It can be replaced without touching anything below it.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod assistant_ui;
pub mod control;
pub mod control_bar;
pub mod credits;
pub mod detached_mixer;
pub mod dialogs;
pub mod docking;
pub mod edit_window;
pub mod extra_windows;
pub mod fonts;
pub mod icons;
pub mod layouts;
pub mod menus;
pub mod midi_editor;
pub mod mix_window;
pub mod nudge_keys;
pub mod ops_windows;
pub mod palette;
pub mod panels;
pub mod plugin_scan;
pub mod score_editor;
pub mod shortcut_profiles;
pub mod shortcuts;
pub mod theme;
pub mod toolbar;
pub mod video_track;
pub mod video_window;
pub mod visual_mixer;
pub mod widgets;
pub mod windows;

use serde_json::{Value, json};
use soundcraft_engine::{Engine, TransportRequest};
use soundcraft_model::TrackId;
use soundcraft_playback::Player;
use soundcraft_time::{Range, Samples};
use std::collections::HashMap;
use std::sync::mpsc::Receiver;

pub use control::ControlRequest;

/// Which main window is in front (Window › Mix / Edit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MainWindow {
    Edit,
    Mix,
    /// Edit with the Recording essentials docked (R18).
    Record,
    /// Navigation shell for the Sprint 4 mastering workflow (R18/R34).
    Master,
}

impl MainWindow {
    pub fn label(self) -> &'static str {
        match self {
            MainWindow::Edit => "Edit",
            MainWindow::Mix => "Mix",
            MainWindow::Record => "Record",
            MainWindow::Master => "Master",
        }
    }

    /// Layouts that show the Edit timeline and its toolbar.
    pub fn is_edit(self) -> bool {
        matches!(self, MainWindow::Edit | MainWindow::Record)
    }
}

/// UI state (serde, so agents can read and set it).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct UiState {
    pub docking: docking::Workspace,
    #[serde(default = "docking::mix_default")]
    pub mix_docking: docking::Workspace,
    pub window: MainWindow,
    pub show_tracks_list: bool,
    pub show_clip_list: bool,
    pub show_transport: bool,
    pub show_memory_locations: bool,
    pub show_big_counter: bool,
    pub show_undo_history: bool,
    pub show_session_info: bool,
    /// HF21: Setup › External Control… (control channel token).
    #[serde(skip)]
    pub show_external_control: bool,
    /// Simple recording panel (R08).
    #[serde(default)]
    pub show_recording: bool,
    /// Window › SML Remote (R21).
    #[serde(default)]
    pub show_remote: bool,
    /// Window › Command Help (R22).
    #[serde(default)]
    pub show_help: bool,
    #[serde(skip)]
    pub help_query: String,
    /// Window › Assistant (R32/R33).
    #[serde(default)]
    pub show_assistant: bool,
    #[serde(skip)]
    pub assistant_input: String,
    /// (request, response) pairs of this session; an executed plan carries `executed`.
    #[serde(skip)]
    pub assistant_log: Vec<(String, serde_json::Value)>,
    /// Last offline meter analysis shown by the Master layout.
    #[serde(skip)]
    pub master_meters: Option<serde_json::Value>,
    pub show_audio_health: bool,
    pub show_about: bool,
    pub show_midi_editor: bool,
    pub show_universe: bool,
    pub narrow_mix: bool,
    /// HF11: the Mixer in its own window, and that window's geometry (a UI preference).
    #[serde(default)]
    pub mixer_window: detached_mixer::MixerWindowPrefs,
    /// Mix window sections: inserts_ae, inserts_fj, sends_ae, sends_fj, io, comments, eq_curve, color.
    pub mix_views: Vec<String>,
    /// Edit window track-header columns: io, inserts_ae, sends_ae, comments, color.
    pub edit_views: Vec<String>,
    /// Open plugin windows (track, slot).
    pub plugin_windows: Vec<(TrackId, usize)>,
    pub audiosuite: Option<String>,
    pub status: String,
    pub show_automation: bool,
    pub show_color_palette: bool,
    pub show_disk_usage: bool,
    pub show_system_usage: bool,
    pub show_task_manager: bool,
    pub show_metadata: bool,
    pub show_event_list: bool,
    pub show_midi_keyboard: bool,
    pub show_workspace: bool,
    pub show_configurations: bool,
    pub show_playback_engine: bool,
    pub show_io_setup: bool,
    pub show_shortcuts: bool,
    pub show_clip_effects: bool,
    pub show_beat_detective: bool,
    pub show_tempo_ops: bool,
    pub show_time_ops: bool,
    pub show_midi_ops: bool,
    pub show_rtp: bool,
    pub show_score: bool,
    pub show_search: bool,
    /// Window › Video, Window › Video Universe, timecode burn-in in the Video window.
    pub show_video: bool,
    pub show_video_universe: bool,
    pub video_burn_in: bool,
    pub workspace_dir: String,
    pub configurations: Vec<(String, Value)>,
    /// Appearance (`ui.theme`): dark by default, light or follow the system on request.
    pub theme: theme::ThemeMode,
    /// Shortcut profile (R13): Pro Tools style, Studio One style or Custom (= `shortcut_base`
    /// plus `shortcut_overrides`, command key → chord; empty chord = unbound).
    /// The user's theme file for `ThemeMode::Custom` (validated; an invalid saved one loads as None).
    #[serde(default, deserialize_with = "theme::lenient_custom")]
    pub theme_custom: Option<theme::ThemeData>,
    /// Monitor-only controls (R37): speaker path, never export; per machine.
    #[serde(default)]
    pub monitor: MonitorState,
    /// SML Remote (R21): the interface to bind when started (loopback by default) and whether
    /// the service starts with the app (opt-in; pairing is always explicit).
    #[serde(default = "default_remote_bind")]
    pub remote_bind: String,
    #[serde(default)]
    pub remote_autostart: bool,
    /// Mix window Split view: share of the height given to the Visual pane (HF01). Saved with
    /// the interface preferences; an invalid value falls back to the default when used.
    #[serde(default = "default_mix_split")]
    pub mix_split: f32,
    #[serde(default)]
    pub shortcut_profile: shortcut_profiles::Profile,
    #[serde(default)]
    pub shortcut_base: shortcut_profiles::Profile,
    #[serde(default)]
    pub shortcut_overrides: std::collections::BTreeMap<String, String>,
    /// Visual mixer channel filter (view only).
    #[serde(skip)]
    pub visual_filter: String,
    /// Visual mixer node drag in progress (start values for Escape-cancel).
    #[serde(skip)]
    pub visual_gesture: Option<visual_mixer::VisualGesture>,
    /// Output devices found by the last scan in the Playback Engine window.
    #[serde(skip)]
    pub output_devices: Vec<soundcraft_playback::device::DeviceCaps>,
    /// Audio Setup driver type (host) the device lists are filtered to (HF07); `None` = the
    /// chosen output's host, else every host.
    #[serde(skip)]
    pub driver_choice: Option<String>,
    /// Requested output device (host, name) and buffer (0 = backend default).
    #[serde(skip)]
    pub device_choice: Option<(String, String)>,
    #[serde(skip)]
    pub buffer_choice: u32,
    /// Input devices found by the last scan, and the input request used for recording
    /// (`None` = the host's default input device).
    #[serde(skip)]
    pub input_devices: Vec<soundcraft_playback::device::DeviceCaps>,
    #[serde(skip)]
    pub input_request: Option<soundcraft_playback::device::DeviceRequest>,
    /// File › Recover Autosave… window.
    #[serde(skip)]
    pub show_recover: bool,
    /// Control bar text being typed (never applied until Enter / Apply): BPM, time signature,
    /// loop Start/End.
    #[serde(skip)]
    pub bpm_draft: Option<String>,
    #[serde(skip)]
    pub meter_draft: Option<(u32, u32)>,
    #[serde(skip)]
    pub loop_draft: Option<[String; 2]>,
    /// Loop lane drag in progress (preview only until release).
    #[serde(skip)]
    pub loop_drag: Option<edit_window::LoopDrag>,
    /// Audio Setup: an input device chosen but not applied (`Some(None)` = default input).
    #[serde(skip)]
    pub input_draft: Option<Option<soundcraft_playback::device::DeviceRequest>>,
    /// Audio Setup opened from this track's input selector.
    #[serde(skip)]
    pub audio_focus_track: Option<TrackId>,
    /// The output saved in Audio Setup could not be reopened at launch: Apply retries it.
    #[serde(skip)]
    pub audio_output_missing: bool,
    /// Device choices applied in Audio Setup, remembered between launches (HF06).
    #[serde(default)]
    pub audio: AudioPrefs,
}

/// Output device, buffer and input device last applied in Audio Setup.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AudioPrefs {
    /// (host, device); `None` = the system default output.
    pub output: Option<(String, String)>,
    /// Requested buffer in frames; 0 = backend default.
    pub buffer: u32,
    /// Input device request; `None` = the system default input.
    pub input: Option<soundcraft_playback::device::DeviceRequest>,
}

/// Autosave status shown in the Recording window and the status bar.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AutosaveState {
    pub last_ok_unix: Option<u64>,
    pub last_generation: Option<u64>,
    pub last_error: Option<String>,
    pub plugin_state_age_s: Option<u64>,
    pub pending: bool,
}

/// A session's newest autosave generation, from its store index.
#[derive(Debug, Clone)]
pub struct RecoveryPoint {
    pub store: soundcraft_engine::autosave::Store,
    pub session_name: String,
    pub generation: u64,
    pub unix_time: u64,
    pub plugin_state_age_s: Option<u64>,
}

/// `HH:MM:SS` (UTC) of a Unix time.
pub fn unix_hms(t: u64) -> String {
    let s = t % 86_400;
    format!("{:02}:{:02}:{:02} UTC", s / 3600, (s / 60) % 60, s % 60)
}

impl Default for UiState {
    fn default() -> Self {
        UiState {
            docking: docking::Workspace::default(),
            mix_docking: docking::mix_default(),
            window: MainWindow::Edit,
            show_tracks_list: true,
            show_clip_list: true,
            show_transport: false,
            show_memory_locations: false,
            show_big_counter: false,
            show_undo_history: false,
            show_session_info: false,
            show_external_control: false,
            show_recording: false,
            show_remote: false,
            show_help: false,
            help_query: String::new(),
            show_assistant: false,
            assistant_input: String::new(),
            assistant_log: Vec::new(),
            master_meters: None,
            show_audio_health: false,
            show_about: false,
            show_midi_editor: false,
            show_universe: false,
            narrow_mix: false,
            mixer_window: detached_mixer::MixerWindowPrefs::default(),
            mix_views: vec!["inserts_ae".into(), "sends_ae".into(), "io".into(), "color".into()],
            edit_views: vec!["color".into()],
            plugin_windows: Vec::new(),
            audiosuite: None,
            status: String::new(),
            show_automation: false,
            show_color_palette: false,
            show_disk_usage: false,
            show_system_usage: false,
            show_task_manager: false,
            show_metadata: false,
            show_event_list: false,
            show_midi_keyboard: false,
            show_workspace: false,
            show_configurations: false,
            show_playback_engine: false,
            show_io_setup: false,
            show_shortcuts: false,
            show_clip_effects: false,
            show_beat_detective: false,
            show_tempo_ops: false,
            show_time_ops: false,
            show_midi_ops: false,
            show_rtp: false,
            show_score: false,
            show_search: false,
            show_video: false,
            show_video_universe: false,
            video_burn_in: false,
            workspace_dir: String::new(),
            configurations: Vec::new(),
            theme: theme::ThemeMode::Dark,
            theme_custom: None,
            monitor: MonitorState::default(),
            remote_bind: default_remote_bind(),
            remote_autostart: false,
            mix_split: default_mix_split(),
            shortcut_profile: shortcut_profiles::Profile::default(),
            shortcut_base: shortcut_profiles::Profile::default(),
            shortcut_overrides: std::collections::BTreeMap::new(),
            visual_filter: String::new(),
            visual_gesture: None,
            output_devices: Vec::new(),
            driver_choice: None,
            device_choice: None,
            buffer_choice: 0,
            input_devices: Vec::new(),
            input_request: None,
            show_recover: false,
            bpm_draft: None,
            meter_draft: None,
            loop_draft: None,
            loop_drag: None,
            input_draft: None,
            audio_focus_track: None,
            audio_output_missing: false,
            audio: AudioPrefs::default(),
        }
    }
}

/// Host services (file dialogs) injected by the app; absent in tests and on the web.
#[derive(Default)]
pub struct Services {
    pub pick_open: Option<Box<dyn Fn(&str, &[&str]) -> Option<String>>>,
    pub pick_save: Option<Box<dyn Fn(&str, &str) -> Option<String>>>,
}

/// File-system-safe folder name for a plugin id (`clap:com.x.y` → `clap_com.x.y`).
pub fn preset_folder_name(plugin: &str) -> String {
    plugin.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' }).collect()
}

/// Drag-and-drop payload: an audio file (source id) dragged from the Clip List.
#[derive(Debug, Clone, Copy)]
pub struct DragSource(pub u64);

/// Ballistic meter display state per strip.
#[derive(Debug, Clone, Copy, Default)]
pub struct MeterDisplay {
    pub level: [f32; 2],
    pub hold: [f32; 2],
    pub hold_age: f32,
    pub clip: bool,
    pub gr: f32,
}

/// An in-progress mouse gesture in the Edit window.
#[derive(Debug, Clone)]
pub enum Gesture {
    Select {
        track: TrackId,
        anchor: Samples,
        tracks: Vec<TrackId>,
    },
    MoveClips {
        clips: Vec<soundcraft_model::ClipId>,
        grab_at: Samples,
        delta: Samples,
        from_track: TrackId,
        to_track: TrackId,
        /// Start of the grabbed clip when the drag began (snapping is measured from it).
        origin: Samples,
        /// Ctrl (Windows/Linux) or Cmd (macOS) held: tracks only, the exact time is kept (HF10).
        time_lock: bool,
        /// Why the current destination is refused (shown on the ghost; nothing moves on release).
        invalid: Option<String>,
    },
    TrimStart {
        clip: soundcraft_model::ClipId,
        track: TrackId,
        to: Samples,
    },
    TrimEnd {
        clip: soundcraft_model::ClipId,
        track: TrackId,
        to: Samples,
    },
    Fade {
        clip: soundcraft_model::ClipId,
        track: TrackId,
        fade_in: bool,
        to: Samples,
    },
    Scrub {
        last: Samples,
    },
    Fader {
        track: TrackId,
        start_db: f32,
    },
    ClipGain {
        clip: soundcraft_model::ClipId,
        track: TrackId,
        pos: f32,
        anchor: (f32, f32),
        fine: bool,
        fader_bottom: egui::Pos2,
    },
    Pencil {
        track: TrackId,
        points: Vec<(Samples, f32)>,
    },
    ZoomBox {
        start: f32,
    },
    /// Eraser sweep (T05): every clip the pointer passed, each id once; committed on release.
    Erase {
        clips: Vec<soundcraft_model::ClipId>,
    },
    /// Mute sweep (T07): the target state comes from the first clip; each id once.
    MuteSweep {
        target: bool,
        clips: Vec<soundcraft_model::ClipId>,
    },
    /// Paint (T06): the drawn range and values; `auto` = the shown automation lane, else the
    /// clip-gain envelope of `clip`.
    Paint {
        track: TrackId,
        clip: Option<soundcraft_model::ClipId>,
        auto: Option<String>,
        start: Samples,
        start_v: f64,
        points: Vec<(Samples, f64)>,
    },
    /// Bend (T08): dragging anchor `index` of `clip` to destination `dst` (clip-relative).
    BendMove {
        clip: soundcraft_model::ClipId,
        index: usize,
        dst: Samples,
    },
    /// Listen (T09): the audition runs while the button is held.
    Listen {
        track: TrackId,
    },
}

fn default_mix_split() -> f32 {
    mix_window::SPLIT_DEFAULT
}

fn default_remote_bind() -> String {
    "127.0.0.1:0".into()
}

/// Monitor-only volume/dim/mono (R37).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct MonitorState {
    pub volume_db: f32,
    pub dim: bool,
    pub mono: bool,
}

impl Default for MonitorState {
    fn default() -> Self {
        MonitorState { volume_db: 0.0, dim: false, mono: false }
    }
}

/// Paint › Transform (T06): the selection box and numeric controls, applied as one command.
#[derive(Debug, Clone)]
pub struct TransformBox {
    pub track: TrackId,
    pub clip: Option<soundcraft_model::ClipId>,
    pub auto: Option<String>,
    pub box_start: Samples,
    pub box_end: Samples,
    pub time_shift: f64,
    pub time_scale: f64,
    pub value_offset: f64,
    pub value_scale: f64,
}

pub struct SoundApp {
    pub engine: Engine,
    pub player: Option<Player>,
    /// Input capture, opened on first record.
    pub recorder: Option<soundcraft_playback::record::Recorder>,
    /// Background plugin scanning (R06).
    pub plugin_scan: plugin_scan::Scanner,
    record_start: Samples,
    /// HF17: where the take may start replacing material (the record point; pre-roll and
    /// count-off before it are timing only).
    record_point: Samples,
    /// Punch range to trim the take to (pre/post-roll recordings).
    punch: Option<Range>,
    /// Count Off: the record point the current take starts at (audio before it is trimmed off).
    countoff_at: Option<Samples>,
    recorder_failed: bool,
    pub ui: UiState,
    pub audio_health_report: Option<Result<Value, String>>,
    pub services: Services,
    /// Set only by a desktop host after installing an OS menu bar.
    pub native_menu_bar: bool,
    /// Folder for crash-recovery autosaves (native apps set it).
    pub autosave_dir: Option<std::path::PathBuf>,
    /// HF21: the control channel's token file and port when started with `--control`.
    pub control_token_path: Option<std::path::PathBuf>,
    pub control_port: Option<u16>,
    /// Folder for user plugin presets (native apps set it).
    pub preset_dir: Option<std::path::PathBuf>,
    last_autosave: f64,
    /// Autosave worker (R04): one job in flight, never on the UI or audio thread.
    autosave_job: Option<std::thread::JoinHandle<Result<soundcraft_engine::autosave::Committed, String>>>,
    pub autosave_state: AutosaveState,
    /// Engine revision of the last snapshot handed to the worker (nothing to save when unchanged).
    last_snapshot_revision: Option<u64>,
    /// When third-party plugin state was last captured into the session (`now` clock).
    last_plugin_capture: Option<f64>,
    pub dialogs: dialogs::Dialogs,
    pub meters: HashMap<TrackId, MeterDisplay>,
    pub main_meter: MeterDisplay,
    pub gesture: Option<Gesture>,
    /// Open Paint › Transform box (numeric controls), if any.
    pub transform: Option<TransformBox>,
    /// Resolved shortcut table for the active profile, rebuilt when the profile or overrides change.
    shortcut_cache: Option<(shortcut_profiles::Profile, shortcut_profiles::Profile, String, Vec<shortcut_profiles::Resolved>)>,
    /// Command id → chord of the active profile (menus read this per item; rebuilt with the cache).
    pub shortcut_labels: HashMap<String, String>,
    /// Keyboard Shortcuts window: the row (override key) waiting for its next chord.
    pub shortcut_capture: Option<String>,
    pub shortcut_query: String,
    /// SML Remote service (R21) while running, and the open pairing code (native builds).
    #[cfg(not(target_arch = "wasm32"))]
    pub remote: Option<soundcraft_remote::Server>,
    pub remote_code: Option<String>,
    remote_snapshot_rev: Option<(u64, bool, bool, Samples)>,
    monitor_applied: Option<MonitorState>,
    pub edit_layout: edit_window::EditLayout,
    pub midi: midi_editor::MidiEditorState,
    pub ops: ops_windows::OpsState,
    pub extra: extra_windows::ExtraState,
    pub palette: palette::PaletteState,
    pub synthetic: Vec<egui::Event>,
    synthetic_grace: u32,
    control_rx: Option<Receiver<ControlRequest>>,
    pending_shots: Vec<control::PendingShot>,
    last_rev: u64,
    fonts_ready: bool,
    fonts_installed: bool,
    /// The palette last applied to the context (light?), so visuals are set only on a change.
    theme_applied: Option<(bool, theme::Tokens)>,
    /// Last panel host context, used to cancel renderer-owned gestures on explicit replacement.
    pub(crate) docking_context: Option<egui::Context>,
    /// Transport simulation when there is no player (tests, offscreen renders).
    sim: Option<(Samples, Option<Samples>, Option<Range>)>,
    pub quit_requested: bool,
    /// Reset floating-window positions next frame (Window › Arrange).
    pub arrange_request: bool,
    /// Tracks whose automation was touched during this pass (Touch/Latch writing).
    touched: std::collections::HashSet<TrackId>,
    last_write_at: Samples,
    pub frame_ms: f32,
    last_frame: Option<f64>,
    /// Movies for the Video window and Video track (decoded in the background).
    pub video: video_track::VideoPool,
    /// Assistant runtime: local model worker, push-to-talk, speech output (RC04/RC05).
    pub assistant: assistant_ui::AssistantRuntime,
    /// Where the Edit toolbar drew its always-reachable groups last frame (RC09).
    pub toolbar_layout: toolbar::ToolbarLayout,
}

/// Whether another automation-write pass is due: the transport moved at least
/// `step` since `last` in either direction. `last == i64::MIN` marks "never
/// written", so the first pass after (re)start is always due. Saturating
/// arithmetic keeps restarts and loop wraps overflow-free: the previous
/// `(pos - last).abs()` panicked in debug builds on the first playing frame,
/// when `last` is still `i64::MIN`.
fn automation_step_due(pos: Samples, last: Samples, step: Samples) -> bool {
    last == i64::MIN || pos.saturating_sub(last) >= step || last.saturating_sub(pos) >= step
}

impl SoundApp {
    pub fn new(engine: Engine, player: Option<Player>, services: Services) -> Self {
        SoundApp {
            engine,
            player,
            recorder: None,
            record_start: 0,
            record_point: 0,
            punch: None,
            countoff_at: None,
            recorder_failed: false,
            plugin_scan: plugin_scan::Scanner::default(),
            ui: UiState::default(),
            audio_health_report: None,
            services,
            native_menu_bar: false,
            autosave_dir: None,
            control_token_path: None,
            control_port: None,
            autosave_job: None,
            autosave_state: AutosaveState::default(),
            last_snapshot_revision: None,
            last_plugin_capture: None,
            preset_dir: None,
            last_autosave: 0.0,
            dialogs: dialogs::Dialogs::default(),
            meters: HashMap::new(),
            main_meter: MeterDisplay::default(),
            gesture: None,
            transform: None,
            shortcut_cache: None,
            shortcut_labels: HashMap::new(),
            shortcut_capture: None,
            shortcut_query: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            remote: None,
            remote_code: None,
            remote_snapshot_rev: None,
            monitor_applied: None,
            edit_layout: edit_window::EditLayout::default(),
            midi: midi_editor::MidiEditorState::default(),
            ops: ops_windows::OpsState::default(),
            extra: extra_windows::ExtraState::default(),
            palette: palette::PaletteState::default(),
            synthetic: Vec::new(),
            synthetic_grace: 0,
            control_rx: None,
            pending_shots: Vec::new(),
            last_rev: 0,
            fonts_ready: false,
            fonts_installed: false,
            theme_applied: None,
            docking_context: None,
            sim: None,
            quit_requested: false,
            arrange_request: false,
            touched: std::collections::HashSet::new(),
            last_write_at: i64::MIN,
            frame_ms: 0.0,
            last_frame: None,
            video: video_track::VideoPool::default(),
            assistant: assistant_ui::AssistantRuntime::default(),
            toolbar_layout: toolbar::ToolbarLayout::default(),
        }
    }

    pub fn with_control(mut self, rx: Receiver<ControlRequest>) -> Self {
        self.control_rx = Some(rx);
        self
    }

    /// Run a command; errors go to the status line. Returns the result.
    pub fn run(&mut self, id: &str, params: Value) -> Result<Value, String> {
        if let Some(r) = menus::run_ui_command(self, id, &params) {
            return r;
        }
        if starts_plugin_scan(id, &params) {
            // HF12: a recovery point before plugin binaries are loaded, even in worker processes.
            if let Err(e) = self.recovery_snapshot_now() {
                log::warn!("recovery point before the plugin scan: {e}");
            }
        }
        if id.starts_with("session.save") || id.starts_with("file.bounce") {
            // Third-party plugins keep state the session can't see: store it first.
            self.capture_plugin_states();
        }
        match self.engine.execute(id, &params) {
            Ok(v) => {
                if id == "app.quit" {
                    self.quit_requested = true;
                }
                if id == "options.scrolling" {
                    // Re-selecting a scrolling mode resumes the playhead follow.
                    self.edit_layout.follow_hold = false;
                }
                Ok(v)
            }
            Err(e) => {
                self.ui.status = e.to_string();
                log::warn!("{id}: {e}");
                Err(e.to_string())
            }
        }
    }

    pub fn is_playing(&self) -> bool {
        self.engine.transport.playing
    }

    #[cfg(target_arch = "wasm32")]
    fn serve_remote(&mut self) {}

    /// SML Remote host side (R21): execute queued envelopes on this thread, publish the
    /// reconnect snapshot when the document or transport changed, feed coalesced meters.
    #[cfg(not(target_arch = "wasm32"))]
    fn serve_remote(&mut self) {
        let Some(server) = self.remote.clone() else { return };
        if self.remote_code.is_some() && server.pairing_remaining().is_none() {
            self.remote_code = None;
        }
        let ran = server.poll(|env| self.submit_remote(env));
        let t = &self.engine.transport;
        let key = (self.engine.revision, t.playing, t.recording, t.position / 4800);
        if ran > 0 || self.remote_snapshot_rev != Some(key) {
            self.remote_snapshot_rev = Some(key);
            server.set_snapshot(self.engine.revision, self.remote_snapshot());
        }
        let tracks: serde_json::Map<String, Value> =
            self.meters.iter().map(|(id, m)| (id.0.to_string(), json!(m.level[0].max(m.level[1])))).collect();
        server.set_meters(json!({"main": [self.main_meter.level[0], self.main_meter.level[1]], "clip": self.main_meter.clip, "tracks": tracks}));
    }

    /// Execute one remote envelope: UI-layer commands (monitor, windows) answer with a receipt
    /// built here; everything else goes through the engine's R09 service (dedup, stale
    /// revision, atomic rejection).
    pub fn submit_remote(&mut self, env: &soundcraft_engine::envelope::Envelope) -> soundcraft_engine::envelope::Receipt {
        use soundcraft_engine::envelope::{Receipt, Rejection};
        if menus::UI_COMMANDS.iter().any(|c| c.0 == env.command) {
            let r = menus::run_ui_command(self, &env.command, &env.params);
            return match r {
                Some(Ok(v)) => Receipt {
                    request_id: env.request_id.clone(),
                    accepted: true,
                    duplicate: false,
                    applied: v,
                    rejection: None,
                    revision: self.engine.revision,
                },
                Some(Err(e)) => Receipt {
                    request_id: env.request_id.clone(),
                    accepted: false,
                    duplicate: false,
                    applied: Value::Null,
                    rejection: Some(Rejection::Failed(e)),
                    revision: self.engine.revision,
                },
                None => Receipt {
                    request_id: env.request_id.clone(),
                    accepted: false,
                    duplicate: false,
                    applied: Value::Null,
                    rejection: Some(Rejection::UnknownCommand),
                    revision: self.engine.revision,
                },
            };
        }
        self.engine.submit(env)
    }

    /// What a phone sees after (re)connecting: enough to render the controls, never media.
    pub fn remote_snapshot(&self) -> Value {
        let s = self.engine.session();
        let t = &self.engine.transport;
        let pos = if t.playing { t.position } else { s.edit.selection.start };
        let position_text = soundcraft_time::format_position(pos, s.edit.main_counter, s.sample_rate, &s.tempo, s.frame_rate, s.timecode_start);
        let tracks: Vec<Value> = s
            .tracks
            .iter()
            .filter(|x| !x.hidden && matches!(x.kind, soundcraft_model::TrackKind::Audio | soundcraft_model::TrackKind::Aux | soundcraft_model::TrackKind::Instrument))
            .map(|x| {
                json!({"id": x.id.0, "name": x.name, "kind": format!("{:?}", x.kind), "volume_db": x.mixer.volume_db, "pan": x.mixer.pan.first().copied().unwrap_or(0.0),
                    "mute": x.mixer.mute, "solo": x.mixer.solo, "arm": x.mixer.record_arm, "monitor": x.mixer.input_monitor, "selected": s.edit.selected_tracks.contains(&x.id)})
            })
            .collect();
        let clip =
            s.edit.selected_clips.first().and_then(|c| s.find_clip(*c)).map(|(_, c)| json!({"id": c.id.0, "name": c.name, "rating": c.rating}));
        json!({
            "session": s.name,
            "transport": {"playing": t.playing, "recording": t.recording, "position": pos, "position_text": position_text},
            "loop": s.edit.loop_playback, "pre_roll": s.edit.pre_post_roll, "countoff": s.edit.countoff,
            "tracks": tracks,
            "markers": s.markers.iter().map(|m| json!({"name": m.name, "at": m.start})).collect::<Vec<_>>(),
            "selected_clip": clip,
            "monitor": {"volume_db": self.ui.monitor.volume_db, "dim": self.ui.monitor.dim, "mono": self.ui.monitor.mono, "cue": self.cue_path()},
            "limits": "HTTP only; no PWA, microphone or encryption claimed; receipts are not sample-accurate execution",
        })
    }

    /// A dedicated cue/headphone output path, if I/O Setup has one beyond the main output
    /// (R37: headphone control appears only for a real cue route).
    pub fn cue_path(&self) -> Option<String> {
        let s = self.engine.session();
        s.outputs
            .iter()
            .skip(1)
            .find(|o| {
                let n = o.name.to_ascii_lowercase();
                n.contains("cue") || n.contains("phone") || n.contains("hp")
            })
            .map(|o| o.name.clone())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn remote_start(&mut self, _bind: Option<&str>) -> Result<Value, String> {
        Err("SML Remote is not available in the web build".into())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn remote_stop(&mut self) {
        self.remote_code = None;
    }

    /// Start the remote service on the configured interface (explicit opt-in).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn remote_start(&mut self, bind: Option<&str>) -> Result<Value, String> {
        if let Some(b) = bind {
            self.ui.remote_bind = b.trim().to_string();
        }
        let addr: std::net::SocketAddr = self
            .ui
            .remote_bind
            .parse()
            .map_err(|e| format!("bind address `{}`: {e} (use host:port, e.g. 127.0.0.1:0 or 0.0.0.0:8787)", self.ui.remote_bind))?;
        if let Some(old) = self.remote.take() {
            old.stop();
        }
        let server = soundcraft_remote::Server::start(soundcraft_remote::Config { bind: addr, ..soundcraft_remote::Config::default() })
            .map_err(|e| format!("cannot bind {addr}: {e}"))?;
        let url = server.url();
        self.remote = Some(server);
        self.remote_code = None;
        self.remote_snapshot_rev = None;
        Ok(json!({"url": url, "https": false, "note": "plain HTTP on the local network; pair with the code from Window › SML Remote"}))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn remote_stop(&mut self) {
        if let Some(s) = self.remote.take() {
            s.stop();
        }
        self.remote_code = None;
    }

    /// The active profile's table (R13).
    pub fn shortcut_table(&self) -> Vec<shortcut_profiles::Entry> {
        shortcut_profiles::table(self.ui.shortcut_profile, self.ui.shortcut_base, &self.ui.shortcut_overrides)
    }

    /// Resolved rows of the active profile, cached until the profile or an override changes.
    pub fn active_shortcuts(&mut self) -> &[shortcut_profiles::Resolved] {
        let fp = soundcraft_engine::autosave::fnv1a64(serde_json::to_string(&self.ui.shortcut_overrides).unwrap_or_default().as_bytes());
        let stale = self.shortcut_cache.as_ref().is_none_or(|(p, b, f, _)| *p != self.ui.shortcut_profile || *b != self.ui.shortcut_base || *f != fp);
        if stale {
            let table = self.shortcut_table();
            self.shortcut_labels = table.iter().filter(|e| e.params.is_null()).map(|e| (e.command.clone(), e.chord.clone())).collect();
            let rows = shortcut_profiles::resolve(&table);
            self.shortcut_cache = Some((self.ui.shortcut_profile, self.ui.shortcut_base, fp, rows));
        }
        self.shortcut_cache.as_ref().map_or(&[], |c| c.3.as_slice())
    }

    /// The chord the active profile gives a command (for menus, search and help).
    pub fn shortcut_for(&self, command: &str, params: Option<&Value>) -> Option<String> {
        let want = match params {
            Some(p) if !p.is_null() => format!("{command}|{p}"),
            _ => command.to_string(),
        };
        self.shortcut_table().into_iter().find(|e| e.key() == want).map(|e| e.chord)
    }

    /// Whether the Pro Tools-style context keys (F-keys, Enter marker, single-key focus) apply.
    pub fn pro_tools_keys(&self) -> bool {
        use shortcut_profiles::Profile;
        match self.ui.shortcut_profile {
            Profile::ProTools => true,
            Profile::StudioOne => false,
            Profile::Custom => self.ui.shortcut_base != Profile::StudioOne,
        }
    }

    /// Current transport position (playhead while playing, else the edit insertion).
    pub fn position(&self) -> Samples {
        if self.engine.transport.playing { self.engine.transport.position } else { self.engine.session().edit.selection.start }
    }

    fn start_play(&mut self, from: Samples, end: Option<Samples>, looped: Option<Range>) {
        if let Some(p) = &self.player {
            p.update_session(self.engine.session_arc());
            p.play(from, end, looped);
        } else {
            self.sim = Some((from, end, looped));
        }
        self.engine.transport.playing = true;
        self.engine.transport.position = from;
        // A fresh start resumes the playhead follow: clear a manual hold and
        // re-sync the follow tracker so the current view is not mistaken for
        // an outside move on the first frame.
        self.edit_layout.follow_hold = false;
        self.edit_layout.last_scroll = self.engine.session().edit.zoom.scroll;
        self.edit_layout.last_follow_to = None;
    }

    fn stop_play(&mut self) {
        if let Some(p) = &self.player {
            p.stop();
            p.set_click_until(None);
        }
        if self.engine.transport.recording {
            self.finish_recording();
        }
        self.sim = None;
        self.engine.transport.playing = false;
        self.engine.transport.recording = false;
        let stop_at = self.engine.transport.position;
        // Snapshot the session flags first: `session_mut` below needs a
        // mutable borrow, so no session borrow may span it.
        let (follow_insertion, after_playback) = {
            let s = self.engine.session();
            (s.edit.insertion_follows_playback, s.edit.scrolling == "after_playback")
        };
        if follow_insertion {
            let pos = self.engine.transport.position;
            self.engine.session_mut().edit.selection = Range::point(pos);
        }
        if after_playback {
            // After Playback never scrolls while playing, so bring the stopped
            // position into view now (only if it is off-screen); every other
            // mode keeps its view.
            let at = stop_at.max(0);
            let (scroll, spp) = {
                let z = &self.engine.session().edit.zoom;
                (z.scroll, z.samples_per_px.max(0.01))
            };
            let width = f64::from(self.edit_layout.timeline[2] - self.edit_layout.timeline[0]).max(0.0);
            let end = scroll.saturating_add((width * spp) as Samples);
            if at < scroll || at >= end {
                let _ = self.engine.execute("view.scroll", &json!({ "to": at }));
            }
        }
        self.edit_layout.follow_hold = false;
    }

    fn start_recording(&mut self) {
        let armed = self.engine.session().tracks.iter().any(|t| t.mixer.record_arm);
        if !armed {
            self.ui.status = "Record-enable a track first (the red button in its header).".into();
            return;
        }
        self.ensure_input();
        let Some(device_channels) = self.recorder.as_ref().map(|r| r.channels) else {
            self.ui.status = "Cannot record: no audio input device".into();
            return;
        };
        // R01/H1: refuse to start rather than record another physical input.
        let problems = soundcraft_engine::io::input_channel_problems(self.engine.session(), device_channels);
        if !problems.is_empty() {
            self.ui.status = format!("Cannot record: {}. Change the track input or the input device.", problems.join("; "));
            return;
        }
        if let Some(p) = &self.player {
            p.set_recording(true);
        }
        let already_playing = self.is_playing();
        if let Some(r) = &self.recorder {
            // Dry capture straight to a new, never-overwritten file beside the session (or in the
            // temp folder for an unsaved session); the callback never holds the take in RAM.
            let dir = self
                .engine
                .path
                .as_ref()
                .and_then(|p| std::path::Path::new(p).parent().map(|d| d.join("Audio Files")))
                .unwrap_or_else(|| std::env::temp_dir().join("sml-recording"));
            let base = self.engine.session().tracks.iter().find(|t| t.mixer.record_arm).map_or_else(|| "Take".to_string(), |t| t.name.clone());
            if let Err(e) = r.start_take(&dir, &base) {
                if let Some(p) = &self.player {
                    p.set_recording(false);
                }
                self.ui.status = format!("Cannot record: {e}");
                return;
            }
        }
        self.engine.transport.recording = true;
        // Count Off applies to a fresh, non-loop start only (set below).
        self.countoff_at = None;
        if already_playing {
            // Punch in on the fly at the current playhead.
            self.record_start = self.position();
            self.record_point = self.record_start;
            self.punch = None;
            return;
        }
        let e = self.engine.session().edit.clone();
        let sel = e.selection;
        if e.loop_record && !sel.is_empty() {
            self.record_start = sel.start;
            self.record_point = sel.start;
            self.punch = None;
            self.start_play(sel.start, None, Some(sel));
        } else {
            // Record into the selection when there is one (punch), with pre/post-roll if enabled;
            // the take is trimmed back to the punch range afterwards.
            let (pre, post) = if e.pre_post_roll { (e.pre_roll, e.post_roll) } else { (0, 0) };
            let mut from = (sel.start - pre).max(0);
            // Count Off (CL-T3): whole bars of click before the record point (click forced on
            // until it, even when the click is off); the take starts at the record point.
            self.countoff_at = None;
            if e.countoff {
                let at = if sel.is_empty() { self.position() } else { sel.start };
                from = from.min(soundcraft_engine::io::countoff_start(self.engine.session(), at, e.countoff_bars));
                self.countoff_at = Some(at);
                if let Some(p) = &self.player {
                    p.set_click_until(Some(at));
                }
            }
            self.record_start = from;
            self.record_point = self.countoff_at.unwrap_or(sel.start);
            self.punch = (!sel.is_empty()).then_some(sel);
            let end = (!sel.is_empty()).then_some(sel.end + post);
            self.start_play(from, end, None);
        }
    }

    /// Reads every live third-party plugin's state from the audio engine and stores it on its
    /// insert (`mix.insert_state`), so saves and bounces include it.
    pub fn capture_plugin_states(&mut self) {
        let Some(p) = &self.player else { return };
        for (t, slot, data) in p.capture_states() {
            let state = soundcraft_model::b64::encode(&data);
            // Dedicated Summing/Tape slot providers store their state on the slot.
            if let Some(kind) = soundcraft_model::SlotKind::ALL.into_iter().find(|k| k.instance_index() == slot) {
                if let Err(e) = self.engine.execute("mix.slot_state", &json!({"track": t.0, "slot": kind.id(), "state": state})) {
                    log::warn!("storing slot plugin state: {e}");
                }
                continue;
            }
            let slot = if slot == soundcraft_mix::INSTRUMENT_SLOT { json!("instrument") } else { json!(slot) };
            if let Err(e) = self.engine.execute("mix.insert_state", &json!({"track": t.0, "slot": slot, "state": state})) {
                log::warn!("storing plugin state: {e}");
            }
        }
    }

    /// Parameter edits made in plugin editors go into the session (one undo step per gesture).
    fn apply_editor_edits(&mut self) {
        let Some(p) = &self.player else { return };
        for (t, slot, param, value) in p.idle() {
            if slot == soundcraft_mix::INSTRUMENT_SLOT {
                continue;
            }
            let key = format!("param:{}:{slot}:{param}", t.0);
            let _ = self.engine.execute_merged("mix.insert_param", &json!({"track": t.0, "slot": slot, "param": param, "value": value}), &key);
        }
    }

    /// Autosave (R04): every `autosave::DEFAULT_INTERVAL_S` seconds a modified session is
    /// snapshotted into its rolling-generation store on a worker thread (recording included: the
    /// callback never waits for it). Hosted plugin state is captured only while the transport is
    /// stopped (asking the audio thread is not realtime-safe), and the snapshot records how old
    /// that capture is.
    fn autosave(&mut self, now: f64) {
        // Collect a finished job first.
        if self.autosave_job.as_ref().is_some_and(|j| j.is_finished())
            && let Some(j) = self.autosave_job.take()
        {
            match j.join() {
                Ok(Ok(c)) => {
                    self.autosave_state.last_ok_unix = Some(c.generation.unix_time);
                    self.autosave_state.last_generation = Some(c.generation.number);
                    self.autosave_state.last_error = None;
                    self.autosave_state.plugin_state_age_s = c.generation.plugin_state_age_s;
                }
                Ok(Err(e)) => self.autosave_state.last_error = Some(e),
                Err(_) => self.autosave_state.last_error = Some("autosave worker panicked".into()),
            }
        }
        let Some(dir) = self.autosave_dir.clone() else { return };
        if self.last_autosave == 0.0 {
            self.last_autosave = now;
        }
        if self.autosave_job.is_some() || now - self.last_autosave < soundcraft_engine::autosave::DEFAULT_INTERVAL_S || !self.engine.is_dirty() {
            return;
        }
        if self.last_snapshot_revision == Some(self.engine.revision) {
            return;
        }
        self.last_autosave = now;
        if !self.is_playing() && !self.engine.transport.recording {
            self.capture_plugin_states();
            self.last_plugin_capture = Some(now);
        }
        let plugin_state_age_s = self.last_plugin_capture.map(|t| (now - t).max(0.0) as u64);
        let req =
            soundcraft_engine::autosave::SnapshotRequest { session: self.engine.session_arc(), revision: self.engine.revision, plugin_state_age_s };
        self.last_snapshot_revision = Some(self.engine.revision);
        let store = soundcraft_engine::autosave::Store::for_session(&dir, &self.engine.session().name);
        self.autosave_state.pending = true;
        let spawned = std::thread::Builder::new().name("sml-autosave".into()).spawn(move || store.snapshot(&req).map_err(|e| e.to_string()));
        match spawned {
            Ok(j) => self.autosave_job = Some(j),
            Err(e) => self.autosave_state.last_error = Some(format!("autosave thread: {e}")),
        }
        self.autosave_state.pending = self.autosave_job.is_some();
    }

    /// Writes a recovery point now (HF12: before a plugin scan) when the session has unsaved
    /// changes, into the same rolling autosave store as the timer does. The named session file is
    /// never written and the session stays modified. `Ok(None)`: nothing to protect (no changes,
    /// or no autosave folder).
    pub fn recovery_snapshot_now(&mut self) -> Result<Option<u64>, String> {
        let Some(dir) = self.autosave_dir.clone() else { return Ok(None) };
        if !self.engine.is_dirty() || self.last_snapshot_revision == Some(self.engine.revision) {
            return Ok(None);
        }
        if !self.is_playing() && !self.engine.transport.recording {
            self.capture_plugin_states();
        }
        let req = soundcraft_engine::autosave::SnapshotRequest {
            session: self.engine.session_arc(),
            revision: self.engine.revision,
            plugin_state_age_s: Some(0),
        };
        let store = soundcraft_engine::autosave::Store::for_session(&dir, &self.engine.session().name);
        let c = store.snapshot(&req).map_err(|e| e.to_string())?;
        self.last_snapshot_revision = Some(self.engine.revision);
        self.autosave_state.last_ok_unix = Some(c.generation.unix_time);
        self.autosave_state.last_generation = Some(c.generation.number);
        self.autosave_state.last_error = None;
        Ok(Some(c.generation.number))
    }

    /// One line for the status areas: last successful recovery point, lag, failure.
    pub fn autosave_summary(&self) -> String {
        let s = &self.autosave_state;
        let mut out = match s.last_generation {
            Some(g) => format!("Autosave: generation {g} committed{}", s.last_ok_unix.map_or(String::new(), |t| format!(" at {}", unix_hms(t)))),
            None if self.autosave_dir.is_some() => "Autosave: no recovery point yet in this run".to_string(),
            None => "Autosave: off (no settings folder)".to_string(),
        };
        if let Some(age) = s.plugin_state_age_s.filter(|a| *a > 0) {
            out.push_str(&format!(" · plugin state {age} s older"));
        }
        if s.pending {
            out.push_str(" · writing…");
        }
        if let Some(e) = &s.last_error {
            out.push_str(&format!(" · FAILED: {e}"));
        }
        out
    }

    /// Marks this run as running (R05): a marker in the autosave folder that a clean exit
    /// removes. Finding it at start means the previous run did not close properly, so the
    /// recovery window is pointed out. Call once after `autosave_dir` is set.
    pub fn mark_running(&mut self) -> bool {
        let Some(dir) = &self.autosave_dir else { return false };
        let marker = dir.join("running.lock");
        let unclean = marker.exists();
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(&marker, format!("{}\n", std::process::id()));
        if unclean {
            self.ui.status =
                "The previous run did not close properly. File › Recover Autosave… lists the recovery points and unfinished takes.".into();
        }
        unclean
    }

    /// Clean exit: removes the running marker.
    pub fn mark_clean_exit(&self) {
        if let Some(dir) = &self.autosave_dir {
            let _ = std::fs::remove_file(dir.join("running.lock"));
        }
    }

    /// Every session store under the autosave folder with its newest generation (index only;
    /// nothing is parsed until the user recovers it).
    pub fn recovery_points(&self) -> Vec<RecoveryPoint> {
        let Some(dir) = &self.autosave_dir else { return Vec::new() };
        let mut out = Vec::new();
        for d in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            if !d.path().is_dir() {
                continue;
            }
            let store = soundcraft_engine::autosave::Store::new(d.path());
            if let Some(g) = store.index().generations.first() {
                out.push(RecoveryPoint {
                    store,
                    session_name: g.session_name.clone(),
                    generation: g.number,
                    unix_time: g.unix_time,
                    plugin_state_age_s: g.plugin_state_age_s,
                });
            }
        }
        out.sort_by_key(|p| std::cmp::Reverse(p.unix_time));
        out
    }

    /// Unfinished takes (capture journals still "recording") beside the open session and in the
    /// temp capture folder.
    pub fn unfinished_takes(&self) -> Vec<soundcraft_playback::capture::UnfinishedTake> {
        let mut dirs = vec![std::env::temp_dir().join("sml-recording")];
        if let Some(p) = &self.engine.path
            && let Some(d) = std::path::Path::new(p).parent()
        {
            dirs.push(d.join("Audio Files"));
        }
        soundcraft_playback::capture::discover_unfinished_takes(&dirs)
    }

    /// Opens a recovery point (the newest valid generation of its store) as an unsaved session.
    pub fn open_recovery_point(&mut self, point: &RecoveryPoint) -> Result<(), String> {
        // HF18: refused while recording (checked first; the take, recorder and session are untouched).
        self.engine.check_session_replace("Recover Autosave").map_err(|e| e.to_string())?;
        let r = point.store.recover().ok_or("no valid generation in this store")?;
        if let Some(p) = &self.player {
            p.stop();
        }
        self.engine.transport.playing = false;
        self.engine.transport.recording = false;
        let missing = soundcraft_engine::autosave::open_recovered(&mut self.engine, r).map_err(|e| e.to_string())?;
        self.recorder = None;
        self.recorder_failed = false;
        self.ui.status = if missing.is_empty() {
            format!("Recovered {} (unsaved — choose File › Save As)", point.session_name)
        } else {
            format!("Recovered {} with {} missing media file(s)", point.session_name, missing.len())
        };
        Ok(())
    }

    /// Recovers an unfinished take to its durable frames and imports it on a new track at 0.
    pub fn recover_take(&mut self, take: &soundcraft_playback::capture::UnfinishedTake) -> Result<(), String> {
        // HF18/K21: a take being recorded is listed as unfinished; recovering it would shorten
        // the file being written.
        self.engine.check_session_replace("Recover a take").map_err(|e| e.to_string())?;
        let rep = soundcraft_playback::capture::recover(&take.media).map_err(|e| e.to_string())?;
        let r = self.run("file.import_audio", json!({"path": take.media.to_string_lossy(), "at": 0}));
        r.map_err(|e| e.to_string())?;
        self.ui.status = format!(
            "Recovered take {}: {} frames kept ({} unjournaled frames discarded); placed at the session start on a new track",
            take.media.display(),
            rep.kept_frames,
            rep.discarded_unjournaled_frames
        );
        Ok(())
    }

    /// Open the input device once (for recording and input monitoring) and feed the player.
    fn ensure_input(&mut self) {
        if self.recorder.is_some() || self.recorder_failed || self.player.is_none() {
            return;
        }
        match soundcraft_playback::record::Recorder::open_with(self.ui.input_request.as_ref()) {
            Ok(r) => {
                if let Some(p) = &self.player {
                    p.set_input(std::sync::Arc::clone(&r.monitor));
                }
                self.recorder = Some(r);
            }
            Err(e) => {
                self.recorder_failed = true;
                self.ui.status = format!("No audio input: {e}");
            }
        }
    }

    /// R01: switch recording to `req` (stopped only). On failure the current input stays open
    /// and the request is not remembered — never a silent substitution.
    /// Audio Setup has choices that differ from what is applied.
    pub fn audio_draft_pending(&self) -> bool {
        self.ui.input_draft.is_some() || self.output_draft_pending()
    }

    fn output_draft_pending(&self) -> bool {
        self.ui.audio_output_missing || self.ui.device_choice != self.ui.audio.output || self.ui.buffer_choice != self.ui.audio.buffer
    }

    /// Audio Setup › Cancel (or closing the window): back to the applied choices.
    pub fn cancel_audio_draft(&mut self) {
        self.ui.device_choice = self.ui.audio.output.clone();
        self.ui.buffer_choice = self.ui.audio.buffer;
        self.ui.input_draft = None;
    }

    /// Audio Setup › Apply (stopped only). The output is opened first with exactly the chosen
    /// device, session rate and buffer; if that fails nothing changes. Then the chosen input.
    /// Applied choices are remembered (`ui.json` `audio`). Returns the status message.
    pub fn apply_audio_draft(&mut self) -> String {
        if self.engine.transport.recording || self.is_playing() {
            return "Stop the transport first: devices change only while stopped".into();
        }
        let mut msgs = Vec::new();
        if self.output_draft_pending() {
            let req = soundcraft_playback::device::DeviceRequest {
                host: self.ui.device_choice.as_ref().map(|c| c.0.clone()),
                device: self.ui.device_choice.as_ref().map(|c| c.1.clone()),
                sample_rate: self.engine.session().sample_rate.hz(),
                buffer_frames: (self.ui.buffer_choice > 0).then_some(self.ui.buffer_choice),
                channels: 2,
            };
            #[cfg(not(target_arch = "wasm32"))]
            match soundcraft_playback::Player::open_requested(self.engine.session_arc(), &req) {
                Ok(p) => {
                    self.player = Some(p);
                    self.ui.audio.output = self.ui.device_choice.clone();
                    self.ui.audio.buffer = self.ui.buffer_choice;
                    self.ui.audio_output_missing = false;
                    msgs.push("Output opened with the requested settings".to_string());
                }
                // The previous device stays open: no silent substitution, and the input is left alone.
                Err(e) => return format!("Device not changed: {e}"),
            }
            #[cfg(target_arch = "wasm32")]
            let _ = req;
        }
        if let Some(choice) = self.ui.input_draft.take() {
            match self.reopen_input(choice.clone()) {
                Ok(()) => {
                    self.ui.audio.input = choice;
                    msgs.push("input opened with the requested settings".to_string());
                }
                Err(e) => msgs.push(format!("input not changed: {e}")),
            }
        }
        if msgs.is_empty() { "Nothing to apply".into() } else { msgs.join("; ") }
    }

    /// At start-up: reopen the output device remembered in Audio Setup, and remember the input
    /// choice for the first recording (the microphone is not opened at launch). A device that
    /// is missing is named in the status line; the system default stays in use and nothing
    /// else is substituted.
    pub fn restore_audio_prefs(&mut self) {
        let a = self.ui.audio.clone();
        self.ui.device_choice = a.output.clone();
        self.ui.buffer_choice = a.buffer;
        self.ui.input_request = a.input.clone();
        if self.player.is_none() || (a.output.is_none() && a.buffer == 0) {
            return;
        }
        let req = soundcraft_playback::device::DeviceRequest {
            host: a.output.as_ref().map(|c| c.0.clone()),
            device: a.output.as_ref().map(|c| c.1.clone()),
            sample_rate: self.engine.session().sample_rate.hz(),
            buffer_frames: (a.buffer > 0).then_some(a.buffer),
            channels: 2,
        };
        #[cfg(not(target_arch = "wasm32"))]
        match soundcraft_playback::Player::open_requested(self.engine.session_arc(), &req) {
            Ok(p) => self.player = Some(p),
            Err(e) => {
                self.ui.audio_output_missing = true;
                let name = a.output.as_ref().map_or("default device".to_string(), |(h, d)| format!("{h}: {d}"));
                self.ui.status = format!(
                    "The output saved in Audio Setup ({name}) could not be opened ({e}); the system default is in use. Reconnect it and press Apply in Audio Setup, or choose another device."
                );
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = req;
    }

    pub fn reopen_input(&mut self, req: Option<soundcraft_playback::device::DeviceRequest>) -> Result<(), String> {
        if self.engine.transport.recording || self.is_playing() {
            return Err("stop the transport to change the input device".into());
        }
        let r = soundcraft_playback::record::Recorder::open_with(req.as_ref())?;
        if let Some(p) = &self.player {
            p.set_input(std::sync::Arc::clone(&r.monitor));
        }
        self.recorder = Some(r);
        self.recorder_failed = false;
        self.ui.input_request = req;
        Ok(())
    }

    fn finish_recording(&mut self) {
        self.engine.transport.recording = false;
        if let Some(p) = &self.player {
            p.set_recording(false);
        }
        let Some(r) = &self.recorder else { return };
        // Read back only the inputs armed tracks record (the file keeps every input).
        let keep = soundcraft_engine::io::armed_input_channels(self.engine.session(), r.channels);
        let take = r.take_selected(&|c| keep.get(c).copied().unwrap_or(false));
        if let Some(rep) = &take.report
            && rep.state.is_partial()
        {
            // Explicit, visible partial capture: every committed frame is kept.
            self.ui.status = format!(
                "Recording stopped early ({:?}): {} frames kept in {}{}",
                rep.state,
                rep.frames,
                rep.media.display(),
                rep.error.as_deref().map(|e| format!(" — {e}")).unwrap_or_default()
            );
        }
        let rate = take.sample_rate;
        // CL04: place the take (or, loop recording, the capture's first frame) from backend
        // timestamps (output presentation anchor vs input capture instant, same host clock) when
        // both exist; otherwise the transport estimate.
        let mut start = self.record_start;
        #[cfg(not(target_arch = "wasm32"))]
        {
            let anchor = self.player.as_ref().and_then(|p| p.play_anchor());
            let first = r.first_capture.lock().ok().and_then(|f| *f);
            if let (Some((play, pos)), Some(cap)) = (anchor, first) {
                let diff = match cap.duration_since(&play) {
                    Some(d) => d.as_secs_f64(),
                    None => play.duration_since(&cap).map_or(0.0, |d| -d.as_secs_f64()),
                };
                let ledger = soundcraft_playback::clock::LatencyLedger {
                    manual: self.engine.session().edit.value("record.offset_samples", 0.0) as i64,
                    ..Default::default()
                };
                let p = soundcraft_playback::clock::place_by_timestamps(pos, diff, self.engine.session().sample_rate.hz(), &ledger);
                log::info!("take placed by timestamps at {} (transport estimate {})", p.session_start, self.record_start);
                start = p.session_start;
            } else {
                log::info!("take placed by transport estimate (backend timestamps unavailable)");
            }
        }
        let sel = self.engine.session().edit.selection;
        if self.engine.session().edit.loop_record && !sel.is_empty() {
            // Split the capture into one take per loop pass; each pass gets its own playlist.
            let made = soundcraft_engine::io::add_loop_recording_at(&mut self.engine, sel, start, &take.channels, rate);
            let session_rate = self.engine.session().sample_rate.hz();
            let note = if rate == session_rate { String::new() } else { format!(" — input ran at {rate} Hz, resampled to {session_rate} Hz") };
            self.ui.status = format!("Loop-recorded {made} take(s){note}");
            return;
        }
        // HF17: only the punch range (or, without one, from the record point on) replaces
        // material; pre-roll, count-off and post-roll are timing. One undo step.
        self.countoff_at = None;
        let keep = match self.punch.take() {
            Some(p) => p,
            None => Range::new(self.record_point, Samples::MAX),
        };
        match soundcraft_engine::io::add_recording_kept(&mut self.engine, start, take.channels, rate, keep) {
            Ok(ids) => {
                let session_rate = self.engine.session().sample_rate.hz();
                self.ui.status = if rate == session_rate {
                    format!("Recorded {} clip(s)", ids.len())
                } else {
                    // Visible, never silent: the take was resampled (the capture file is unchanged).
                    format!("Recorded {} clip(s) — input ran at {rate} Hz, resampled to the session's {session_rate} Hz", ids.len())
                };
            }
            Err(e) => self.ui.status = e.to_string(),
        }
    }

    /// While playing, record volume automation from fader positions for tracks in Write mode, and
    /// for Touch/Latch tracks whose fader is (or, for Latch, was) being moved. One undo step per pass.
    fn write_automation(&mut self, ctx: &egui::Context) {
        use soundcraft_model::AutomationMode as M;
        if !self.is_playing() {
            if !self.touched.is_empty() {
                self.touched.clear();
                self.engine.end_merge();
            }
            self.last_write_at = i64::MIN;
            return;
        }
        let pos = self.position();
        let step = self.engine.session().sample_rate.samples(0.02);
        if !automation_step_due(pos, self.last_write_at, step) {
            return;
        }
        self.last_write_at = pos;
        let dragging = ctx.input(|i| i.pointer.any_down());
        let fader_track = match &self.gesture {
            Some(Gesture::Fader { track, .. }) => Some(*track),
            _ => None,
        };
        let writes: Vec<(TrackId, f32)> = self
            .engine
            .session()
            .tracks
            .iter()
            .filter_map(|t| {
                let m = t.mixer.automation_mode;
                let touched_now = dragging && fader_track == Some(t.id);
                let write = match m {
                    M::Write => true,
                    M::Touch => touched_now,
                    M::Latch | M::TouchLatch => touched_now || self.touched.contains(&t.id),
                    _ => false,
                };
                write.then_some((t.id, t.mixer.volume_db))
            })
            .collect();
        for (t, v) in writes {
            self.touched.insert(t);
            let _ = self.engine.execute_merged(
                "automation.set_point",
                &serde_json::json!({"track": t.0, "param": "volume", "at": pos, "value": v}),
                "automation_pass",
            );
        }
    }

    /// Play from the selection, honouring loop playback and selection end.
    pub fn play_from_selection(&mut self) {
        let s = self.engine.session();
        let sel = s.edit.selection;
        let preroll = if s.edit.pre_post_roll { s.edit.pre_roll } else { 0 };
        let postroll = if s.edit.pre_post_roll { s.edit.post_roll } else { 0 };
        // HF05: stored loop locators win; without them the selection loops, as before.
        if s.edit.loop_playback
            && let Some((range, _)) = soundcraft_engine::cmd::loop_effective_range(s)
        {
            self.start_play(range.start, None, Some(range));
        } else if !sel.is_empty() {
            self.start_play((sel.start - preroll).max(0), Some(sel.end + postroll), None);
        } else {
            self.start_play((sel.start - preroll).max(0), None, None);
        }
    }

    fn handle_transport(&mut self, dt: f32) {
        let reqs: Vec<TransportRequest> = std::mem::take(&mut self.engine.transport_requests);
        for r in reqs {
            match r {
                TransportRequest::ResetSession => {
                    if let Some(p) = &self.player {
                        p.stop();
                        p.set_recording(false);
                        p.set_speed(1.0);
                        p.locate(self.engine.transport.position);
                    }
                    self.sim = None;
                    // The old capture belongs to the replaced document, never to the new one.
                    self.recorder = None;
                    self.recorder_failed = false;
                    self.punch = None;
                    self.gesture = None;
                    self.touched.clear();
                    self.last_write_at = i64::MIN;
                    self.edit_layout.follow_hold = false;
                    self.meters.clear();
                    self.main_meter = MeterDisplay::default();
                }
                TransportRequest::Play => {
                    if !self.is_playing() {
                        self.play_from_selection();
                    }
                }
                TransportRequest::Stop | TransportRequest::Pause => self.stop_play(),
                TransportRequest::TogglePlay => {
                    if self.is_playing() {
                        self.stop_play();
                    } else {
                        self.play_from_selection();
                    }
                }
                TransportRequest::Record => {
                    if self.engine.transport.recording {
                        self.stop_play();
                    } else {
                        self.start_recording();
                    }
                }
                TransportRequest::Locate(at) => {
                    if self.is_playing() {
                        if let Some(p) = &self.player {
                            p.locate(at);
                        }
                        if let Some(sim) = &mut self.sim {
                            sim.0 = at;
                        }
                    }
                    self.engine.transport.position = at;
                }
                TransportRequest::PlaySelection => {
                    let sel = self.engine.session().edit.selection;
                    self.start_play(sel.start, Some(sel.end.max(sel.start + 1)), None);
                }
                TransportRequest::HalfSpeed => {
                    if let Some(p) = &self.player {
                        p.set_speed(0.5);
                    }
                    self.play_from_selection();
                }
                TransportRequest::Scrub(at) => self.engine.transport.position = at,
                TransportRequest::AllNotesOff => {}
            }
        }
        // Read back the position.
        if let Some(p) = &self.player {
            let playing = p.is_playing();
            let ended = self.engine.transport.playing && !playing;
            if ended {
                p.set_speed(1.0);
            }
            if self.engine.transport.playing {
                self.engine.transport.position = p.position();
            }
            let snap = p.meters();
            for (id, m) in &snap.tracks {
                let d = self.meters.entry(*id).or_default();
                feed_meter(d, m.peak, m.gain_reduction_db, dt);
            }
            let main = snap.main.peak;
            feed_meter(&mut self.main_meter, main, 0.0, dt);
            if ended {
                self.stop_play();
            }
        } else if let Some((pos, end, looped)) = &mut self.sim {
            let sr = self.engine.session().sample_rate.as_f64();
            *pos += (f64::from(dt) * sr) as i64;
            if let Some(l) = looped
                && *pos >= l.end
            {
                *pos = l.start;
            }
            let stop = end.is_some_and(|e| *pos >= e);
            self.engine.transport.position = *pos;
            if stop {
                self.stop_play();
            }
        }
        if !self.engine.transport.playing {
            for d in self.meters.values_mut() {
                feed_meter(d, [0.0, 0.0], 0.0, dt);
            }
            feed_meter(&mut self.main_meter, [0.0, 0.0], 0.0, dt);
        }
    }

    /// Per-frame logic: control channel, transport, document sync.
    pub fn logic(&mut self, ctx: &egui::Context) {
        // Plugin scanning runs in the background, never while recording.
        if self.plugin_scan.phase() == plugin_scan::Phase::Idle {
            self.plugin_scan.start(self.engine.transport.recording);
        }
        // Fonts set now take effect next frame, so draw only from the frame after.
        if !self.fonts_ready {
            if self.fonts_installed {
                self.fonts_ready = true;
            } else {
                fonts::install(ctx);
                self.fonts_installed = true;
                ctx.request_repaint();
            }
        }
        let (light, tokens) = theme::resolve(ctx, self.ui.theme, self.ui.theme_custom.as_ref());
        if self.theme_applied != Some((light, tokens)) || theme::is_light() != light || theme::Tokens::current() != tokens {
            theme::apply_tokens(ctx, light, tokens);
            self.theme_applied = Some((light, tokens));
        }
        let now = ctx.input(|i| i.time);
        let dt = self.last_frame.map_or(1.0 / 60.0, |t| (now - t) as f32).clamp(0.0, 0.25);
        self.last_frame = Some(now);
        self.frame_ms = self.frame_ms * 0.9 + dt * 1000.0 * 0.1;
        control::drain(self, ctx);
        self.apply_editor_edits();
        // T05/T07/T09/T10: Escape or focus loss cancels a tool sweep without committing it,
        // ends a Listen audition and clears temporary tool state.
        let (esc, focused, pointer_down) = ctx.input(|i| (i.key_pressed(egui::Key::Escape), i.focused, i.pointer.any_down()));
        if (esc || !focused)
            && matches!(self.gesture, Some(Gesture::Erase { .. } | Gesture::MuteSweep { .. } | Gesture::Paint { .. } | Gesture::BendMove { .. }))
        {
            self.gesture = None;
        }
        if esc {
            self.transform = None;
        }
        if self.engine.session().listen.active() && (esc || !focused || !pointer_down) {
            let _ = self.engine.execute("audition.stop", &json!({}));
        }
        // The Listen gesture ends with the button, so a cancelled audition cannot restart while
        // the button is still held.
        if !pointer_down && matches!(self.gesture, Some(Gesture::Listen { .. })) {
            self.gesture = None;
        }
        // RC04/RC05: model answers, recognised speech and queued speech output.
        self.assistant_poll();
        if self.assistant.is_busy() || self.assistant.is_recording() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        // T08: finished Bend renders land here (never on the audio thread); stale ones are dropped.
        if self.engine.warp.is_busy() {
            let _ = self.engine.execute("clip.warp_poll", &json!({}));
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        if !pointer_down {
            if matches!(self.gesture, Some(Gesture::Fader { .. })) {
                self.gesture = None;
            }
            // Keep a running automation pass merged until playback stops.
            if !ctx.egui_wants_keyboard_input() && self.touched.is_empty() {
                self.engine.end_merge();
            }
        }
        // CL04: plugin latency changed while recording — finalize the committed capture, stop
        // with a named error; resuming is an explicit user action.
        if self.engine.transport.recording && self.player.as_ref().is_some_and(|p| p.take_latency_alarm()) {
            self.finish_recording();
            if let Some(p) = &self.player {
                p.stop();
            }
            self.engine.transport.playing = false;
            self.ui.status = "Recording stopped: plugin latency changed during the take (alignment would be wrong). The take so far was kept; press Record to continue.".into();
        }
        // CL-T2: the input stream failed (interface unplugged / driver error). A running take is
        // finalized as an explicit partial (frames before the failure kept) and the transport
        // stops; the dead stream is dropped so the next Record reopens the input.
        if let Some(why) = self.recorder.as_ref().and_then(|r| r.input_lost()) {
            if self.engine.transport.recording {
                self.finish_recording();
                if let Some(p) = &self.player {
                    p.stop();
                }
                self.engine.transport.playing = false;
                self.ui.status = format!("Recording stopped: {why}. {}", self.ui.status);
            } else {
                self.ui.status = format!("Audio input stopped: {why}.");
            }
            self.recorder = None;
            self.recorder_failed = false;
        }
        // The output stream failed: the transport cannot advance. A running take (input is
        // separate) is finalized normally; the user reopens the device in Playback Engine.
        if self.player.as_ref().and_then(|p| p.output_lost()).is_some() && (self.engine.transport.playing || self.engine.transport.recording) {
            let why = self.player.as_ref().and_then(|p| p.output_lost()).unwrap_or_default();
            let take = if self.engine.transport.recording {
                self.finish_recording();
                format!(" {}", self.ui.status)
            } else {
                String::new()
            };
            if let Some(p) = &self.player {
                p.stop();
            }
            self.engine.transport.playing = false;
            self.ui.status = format!("Playback stopped: {why}. Reopen the device in Setup › Playback Engine.{take}");
        }
        // Stop the old playback before sending a replacement session to the audio thread.
        self.handle_transport(dt);
        if self.engine.revision != self.last_rev {
            self.last_rev = self.engine.revision;
            if let Some(p) = &self.player {
                p.update_session(self.engine.session_arc());
            }
        }
        self.write_automation(ctx);
        self.autosave(now);
        // R37: monitor-only controls reach the device output stage only.
        if self.monitor_applied != Some(self.ui.monitor) {
            if let Some(p) = &self.player {
                p.set_monitor(self.ui.monitor.volume_db, self.ui.monitor.dim, self.ui.monitor.mono);
            }
            self.monitor_applied = Some(self.ui.monitor);
        }
        self.serve_remote();
        if self.recorder.is_none()
            && !self.recorder_failed
            && self.engine.session().tracks.iter().any(|t| t.mixer.input_monitor || t.mixer.record_arm)
        {
            self.ensure_input();
        }
        if self.is_playing() || self.meters.values().any(|m| m.level[0] > 0.0001) || !self.pending_shots.is_empty() || !self.synthetic.is_empty() {
            ctx.request_repaint();
        }
        control::collect_screenshots(self, ctx);
    }

    /// Feed queued synthetic input (from the control channel) into egui: one pointer event per
    /// frame (so drags span frames like real ones), keys as press+release pairs, text at once.
    pub fn raw_input_hook(&mut self, raw: &mut egui::RawInput) {
        // While a synthetic gesture plays (and briefly after), the real pointer is ignored so it
        // cannot interleave with the scripted one.
        if !self.synthetic.is_empty() {
            self.synthetic_grace = 6;
        }
        if self.synthetic_grace > 0 {
            self.synthetic_grace -= 1;
            raw.events.retain(|e| {
                !matches!(e, egui::Event::PointerMoved(_) | egui::Event::PointerButton { .. } | egui::Event::PointerGone | egui::Event::MouseMoved(_))
            });
        }
        let Some(first) = self.synthetic.first().cloned() else { return };
        let take = match first {
            egui::Event::Key { pressed: true, .. } => {
                // Press and the matching release together.
                if matches!(self.synthetic.get(1), Some(egui::Event::Key { pressed: false, .. })) { 2 } else { 1 }
            }
            _ => 1,
        };
        let n = take.min(self.synthetic.len());
        let evs: Vec<egui::Event> = self.synthetic.drain(..n).collect();
        for e in &evs {
            if let egui::Event::PointerButton { pos, .. } | egui::Event::PointerMoved(pos) = e {
                // Keep the hover position current for widgets that check it.
                raw.events.push(egui::Event::PointerMoved(*pos));
            }
        }
        raw.events.extend(evs);
    }

    /// Lay out the whole window.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        // The integration creates this Ui before logic resolves the current frame's palette.
        ui.set_style(ctx.global_style());
        if !self.fonts_ready {
            ctx.request_repaint();
            return;
        }
        if self.arrange_request {
            self.arrange_request = false;
            ctx.memory_mut(|m| m.reset_areas());
        }
        shortcuts::handle(self, &ctx);
        if !self.native_menu_bar {
            menus::menu_bar(self, ui);
        }
        control_bar::show(self, ui);
        match self.ui.window {
            MainWindow::Edit => edit_window::show(self, ui),
            MainWindow::Mix if self.ui.mixer_window.detached => detached_mixer::placeholder(self, ui),
            MainWindow::Mix => mix_window::show(self, ui),
            MainWindow::Record => layouts::record(self, ui),
            MainWindow::Master => layouts::master(self, ui),
        }
        detached_mixer::show(self, &ctx);
        panels::floating(self, &ctx);
        windows::show(self, &ctx);
        video_window::show(self, &ctx);
        ops_windows::show(self, &ctx);
        extra_windows::show(self, &ctx);
        score_editor::show(self, &ctx);
        palette::show(self, &ctx);
        dialogs::show(self, &ctx);
    }

    /// UI state as JSON (control channel `ui.inspect`).
    pub fn inspect(&self, ctx: &egui::Context) -> Value {
        let size = ctx.content_rect().size();
        json!({
            "ui": self.ui,
            "window_size": [size.x, size.y],
            "playing": self.is_playing(),
            "position": self.position(),
            "dialog": self.dialogs.open_name(),
            "frame_ms": self.frame_ms,
            "audio_device": self.player.as_ref().map(|p| p.device_name.clone()),
            "status": self.ui.status,
        })
    }
}

fn feed_meter(d: &mut MeterDisplay, peak: [f32; 2], gr: f32, dt: f32) {
    // Instant attack, ~26 dB/s release, 2 s peak hold.
    let fall = 10f32.powf(-26.0 * dt / 20.0);
    for c in 0..2 {
        let v = peak.get(c).copied().unwrap_or(0.0);
        let lvl = d.level.get(c).copied().unwrap_or(0.0);
        let nv = if v >= lvl { v } else { (lvl * fall).max(v) };
        if let Some(x) = d.level.get_mut(c) {
            *x = if nv < 1e-5 { 0.0 } else { nv };
        }
        if v >= d.hold.get(c).copied().unwrap_or(0.0) {
            if let Some(h) = d.hold.get_mut(c) {
                *h = v;
            }
            d.hold_age = 0.0;
        }
        if v >= 1.0 {
            d.clip = true;
        }
    }
    d.hold_age += dt;
    if d.hold_age > 2.0 {
        d.hold = d.level;
    }
    d.gr = if gr > d.gr { gr } else { d.gr * fall };
}

/// Whether command `id` starts a plugin scan job (a recovery point is taken first, HF12).
pub fn starts_plugin_scan(id: &str, params: &Value) -> bool {
    match id {
        "engine.plugin_scan_start" | "engine.plugin_scan_retry" => true,
        "engine.vst3_plugins" | "engine.clap_plugins" => params.get("rescan").and_then(Value::as_bool).unwrap_or(false),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{Services, SoundApp, automation_step_due};
    use serde_json::json;

    #[test]
    fn new_session_stops_playback_and_the_old_clock() {
        for (command, params) in [
            ("session.new", json!({"name": "New project", "sample_rate": 96_000})),
            ("session.new", json!({"template": "demo"})),
            ("session.close", json!({})),
        ] {
            let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
            app.run("transport.play", json!({"from": 48_000})).unwrap();
            app.handle_transport(0.25);
            assert!(app.is_playing());
            assert!(app.position() > 48_000);
            app.run(command, params).unwrap();
            let insertion = app.engine.session().edit.selection.start;
            app.handle_transport(0.25);
            assert!(!app.is_playing(), "{command}");
            assert_eq!(app.position(), insertion, "{command}");
            assert_eq!(app.engine.transport.position, insertion, "{command}");
            app.handle_transport(0.25);
            assert_eq!(app.engine.transport.position, insertion, "the old clock must stay stopped");
            app.run("transport.play", json!({})).unwrap();
            app.handle_transport(0.25);
            assert!(app.is_playing());
            assert_eq!(app.position(), insertion + i64::from(app.engine.session().sample_rate.hz()) / 4);
        }
    }

    /// R04: the autosave worker commits a generation of a modified session into its store;
    /// an unchanged revision is not snapshotted again; the running marker reports an unclean exit.
    #[test]
    fn autosave_worker_commits_generations_and_marks_unclean_exit() {
        let dir = std::env::temp_dir().join(format!("sml-ui-autosave-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
        app.autosave_dir = Some(dir.clone());
        assert!(!app.mark_running(), "first run: no stale marker");
        app.run("mix.volume", json!({"track": "Kick", "db": -4.0})).unwrap();
        app.autosave(1.0);
        app.autosave(1.0 + soundcraft_engine::autosave::DEFAULT_INTERVAL_S + 1.0);
        assert!(app.autosave_job.is_some() || app.autosave_state.last_generation.is_some(), "a job was started");
        for _ in 0..600 {
            app.autosave(1000.0);
            if app.autosave_state.last_generation.is_some() || app.autosave_state.last_error.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(app.autosave_state.last_error, None);
        assert_eq!(app.autosave_state.last_generation, Some(1));
        assert!(app.autosave_summary().contains("generation 1"));
        // Same revision: no second generation even after the interval.
        app.autosave(2000.0 + soundcraft_engine::autosave::DEFAULT_INTERVAL_S);
        assert!(app.autosave_job.is_none());
        let points = app.recovery_points();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].generation, 1);
        // A second "run" finds the marker: unclean exit reported; a clean exit removes it.
        assert!(app.mark_running(), "marker from the unfinished run");
        app.mark_clean_exit();
        assert!(!dir.join("running.lock").exists());
        // Recovery opens the generation as an unsaved session.
        let mut fresh = SoundApp::new(soundcraft_engine::Engine::default(), None, Services::default());
        fresh.open_recovery_point(&points[0]).unwrap();
        assert!(fresh.engine.path.is_none() && fresh.engine.is_dirty());
        assert_eq!(fresh.engine.session().track_by_name("Kick").map(|t| t.mixer.volume_db), Some(-4.0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn failed_input_switch_keeps_the_current_input_and_request() {
        let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
        let missing = soundcraft_playback::device::DeviceRequest {
            host: Some("NoSuchHost".into()),
            device: Some("No Such Interface".into()),
            sample_rate: 48_000,
            buffer_frames: Some(64),
            channels: 2,
        };
        let err = app.reopen_input(Some(missing)).unwrap_err();
        assert!(err.contains("NoSuchHost"), "{err}");
        assert!(app.ui.input_request.is_none(), "a failed request is not remembered");
        assert!(app.recorder.is_none(), "no other device was substituted");
        app.run("transport.play", json!({})).unwrap();
        app.handle_transport(0.01);
        assert!(app.reopen_input(None).unwrap_err().contains("stop the transport"));
    }

    #[test]
    fn new_session_cancels_play_queued_before_the_next_frame() {
        let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
        app.run("transport.play", json!({})).unwrap();
        app.run("session.new", json!({})).unwrap();
        app.handle_transport(0.25);
        assert!(!app.is_playing());
        assert_eq!(app.engine.transport.position, 0);
    }

    #[test]
    fn session_reset_does_not_apply_old_stop_edits_to_the_new_session() {
        let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
        app.run("transport.play", json!({"from": 48_000})).unwrap();
        app.handle_transport(0.25);
        let mut session = soundcraft_model::Session::default();
        session.edit.selection = soundcraft_time::Range::new(12_000, 24_000);
        session.edit.insertion_follows_playback = true;
        session.edit.scrolling = "after_playback".into();
        let selection = session.edit.selection;
        app.engine.replace_session(session);
        app.handle_transport(0.25);
        assert!(!app.is_playing());
        assert_eq!(app.engine.session().edit.selection, selection);
        assert_eq!(app.position(), selection.start);
        assert_eq!(app.engine.session().edit.zoom.scroll, 0);
        assert!(!app.engine.is_dirty());
    }

    #[test]
    fn automation_first_pass_after_start_is_due_without_overflow() {
        // `last_write_at` starts (and resets) at `i64::MIN`: the previous
        // `(pos - last).abs()` panicked here in debug builds.
        assert!(automation_step_due(0, i64::MIN, 960));
        assert!(automation_step_due(48_000, i64::MIN, 960));
    }

    #[test]
    fn automation_write_throttles_to_step() {
        assert!(!automation_step_due(1_000, 900, 960));
        assert!(automation_step_due(2_000, 900, 960));
    }

    #[test]
    fn automation_write_fires_after_loop_wrap() {
        assert!(automation_step_due(0, 1_000_000, 960));
    }
}
