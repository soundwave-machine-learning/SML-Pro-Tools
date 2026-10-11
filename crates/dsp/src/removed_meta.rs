//! Metadata of the removed upstream bundled processors (ids, names, categories, parameter
//! tables) — kept only so old sessions can show their inactive missing placeholders by name and
//! keep their parameter values. No processing code remains; nothing here can be instantiated.

#![allow(dead_code)]

pub mod drums {
    #[allow(unused_imports)]
    use crate::params::{choice, lin, log, toggle};
    #[allow(unused_imports)]
    use crate::{Category, ParamInfo, PluginInfo, Unit};
    static DRUM_PARAMS: [ParamInfo; 11] = [
        lin("level", "Level", -48.0, 6.0, -6.0, Unit::Db),
        lin("kick_tune", "Kick Tune", -12.0, 12.0, 0.0, Unit::Semitones),
        log("kick_decay", "Kick Decay", 50.0, 2000.0, 450.0, Unit::Ms),
        lin("snare_tune", "Snare Tune", -12.0, 12.0, 0.0, Unit::Semitones),
        lin("snare_snappy", "Snare Snappy", 0.0, 100.0, 60.0, Unit::Percent),
        log("snare_decay", "Snare Decay", 50.0, 1000.0, 220.0, Unit::Ms),
        log("hat_decay", "Closed Hat Decay", 20.0, 500.0, 80.0, Unit::Ms),
        log("open_hat_decay", "Open Hat Decay", 100.0, 3000.0, 600.0, Unit::Ms),
        lin("tom_tune", "Tom Tune", -12.0, 12.0, 0.0, Unit::Semitones),
        log("tom_decay", "Tom Decay", 50.0, 2000.0, 500.0, Unit::Ms),
        log("cymbal_decay", "Cymbal Decay", 200.0, 6000.0, 2000.0, Unit::Ms),
    ];
    pub static DRUM_INFO: PluginInfo = PluginInfo {
        id: "drum_synth",
        name: "Drum Synth",
        short_name: "Drums",
        category: Category::Instrument,
        params: &DRUM_PARAMS,
        is_instrument: true,
        audiosuite: false,
    };
    const VOICES: usize = 24;
    const OFF: f32 = 1.0e-4;
}
pub mod dynamics {
    #[allow(unused_imports)]
    use crate::params::{choice, lin, log, toggle};
    #[allow(unused_imports)]
    use crate::{Category, ParamInfo, PluginInfo, Unit};
    static COMP_PARAMS: [ParamInfo; 11] = [
        lin("threshold", "Threshold", -60.0, 0.0, -20.0, Unit::Db),
        log("ratio", "Ratio", 1.0, 100.0, 4.0, Unit::Ratio),
        log("attack", "Attack", 0.01, 300.0, 10.0, Unit::Ms),
        log("release", "Release", 5.0, 3000.0, 100.0, Unit::Ms),
        lin("knee", "Knee", 0.0, 24.0, 6.0, Unit::Db),
        lin("makeup", "Makeup", 0.0, 40.0, 0.0, Unit::Db),
        toggle("sc_hpf_on", "SC HPF", false),
        log("sc_hpf_freq", "SC HPF Freq", 20.0, 500.0, 100.0, Unit::Hz),
        toggle("limiter", "Limit", false),
        lin("lookahead", "Lookahead", 0.0, 10.0, 0.0, Unit::Ms),
        lin("mix", "Mix", 0.0, 100.0, 100.0, Unit::Percent),
    ];
    pub static COMP_INFO: PluginInfo = PluginInfo {
        id: "compressor",
        name: "Compressor/Limiter",
        short_name: "Comp",
        category: Category::Dynamics,
        params: &COMP_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
    static GATE_PARAMS: [ParamInfo; 6] = [
        lin("threshold", "Threshold", -80.0, 0.0, -40.0, Unit::Db),
        log("ratio", "Ratio", 1.0, 100.0, 100.0, Unit::Ratio),
        lin("range", "Range", 0.0, 80.0, 80.0, Unit::Db),
        log("attack", "Attack", 0.01, 100.0, 0.5, Unit::Ms),
        lin("hold", "Hold", 0.0, 2000.0, 50.0, Unit::Ms),
        log("release", "Release", 1.0, 4000.0, 100.0, Unit::Ms),
    ];
    pub static GATE_INFO: PluginInfo = PluginInfo {
        id: "expander_gate",
        name: "Expander/Gate",
        short_name: "Gate",
        category: Category::Dynamics,
        params: &GATE_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
    static DEESS_PARAMS: [ParamInfo; 4] = [
        log("freq", "Freq", 2000.0, 16000.0, 6000.0, Unit::Hz),
        lin("range", "Range", 0.0, 40.0, 10.0, Unit::Db),
        lin("threshold", "Threshold", -60.0, 0.0, -30.0, Unit::Db),
        toggle("listen", "Listen", false),
    ];
    pub static DEESS_INFO: PluginInfo = PluginInfo {
        id: "de_esser",
        name: "De-Esser",
        short_name: "DeEss",
        category: Category::Dynamics,
        params: &DEESS_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
    static MAX_PARAMS: [ParamInfo; 3] = [
        lin("threshold", "Threshold", -30.0, 0.0, 0.0, Unit::Db),
        lin("ceiling", "Ceiling", -30.0, 0.0, -0.3, Unit::Db),
        log("release", "Release", 1.0, 1000.0, 50.0, Unit::Ms),
    ];
    pub static MAX_INFO: PluginInfo = PluginInfo {
        id: "maximizer",
        name: "Maximizer",
        short_name: "Max",
        category: Category::Dynamics,
        params: &MAX_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
    const MAX_LOOKAHEAD_MS: f32 = 1.5;
}
pub mod eq {
    #[allow(unused_imports)]
    use crate::params::{choice, lin, log, toggle};
    #[allow(unused_imports)]
    use crate::{Category, ParamInfo, PluginInfo, Unit};
    const SLOPES: &[&str] = &["6 dB/oct", "12 dB/oct", "18 dB/oct", "24 dB/oct"];
    static EQ7_PARAMS: [ParamInfo; 23] = [
        lin("input_gain", "Input", -24.0, 24.0, 0.0, Unit::Db),
        toggle("hpf_on", "HPF", false),
        log("hpf_freq", "HPF Freq", 10.0, 2000.0, 80.0, Unit::Hz),
        choice("hpf_slope", "HPF Slope", SLOPES, 1),
        log("low_shelf_freq", "LF Freq", 20.0, 1000.0, 100.0, Unit::Hz),
        lin("low_shelf_gain", "LF Gain", -24.0, 24.0, 0.0, Unit::Db),
        log("low_shelf_q", "LF Q", 0.3, 2.0, 0.707, Unit::None),
        log("low_mid_freq", "LMF Freq", 20.0, 2000.0, 250.0, Unit::Hz),
        lin("low_mid_gain", "LMF Gain", -24.0, 24.0, 0.0, Unit::Db),
        log("low_mid_q", "LMF Q", 0.1, 10.0, 1.0, Unit::None),
        log("mid_freq", "MF Freq", 100.0, 8000.0, 1000.0, Unit::Hz),
        lin("mid_gain", "MF Gain", -24.0, 24.0, 0.0, Unit::Db),
        log("mid_q", "MF Q", 0.1, 10.0, 1.0, Unit::None),
        log("high_mid_freq", "HMF Freq", 500.0, 20000.0, 4000.0, Unit::Hz),
        lin("high_mid_gain", "HMF Gain", -24.0, 24.0, 0.0, Unit::Db),
        log("high_mid_q", "HMF Q", 0.1, 10.0, 1.0, Unit::None),
        log("high_shelf_freq", "HF Freq", 1000.0, 20000.0, 8000.0, Unit::Hz),
        lin("high_shelf_gain", "HF Gain", -24.0, 24.0, 0.0, Unit::Db),
        log("high_shelf_q", "HF Q", 0.3, 2.0, 0.707, Unit::None),
        toggle("lpf_on", "LPF", false),
        log("lpf_freq", "LPF Freq", 1000.0, 20000.0, 18000.0, Unit::Hz),
        choice("lpf_slope", "LPF Slope", SLOPES, 1),
        lin("output_gain", "Output", -24.0, 24.0, 0.0, Unit::Db),
    ];
    pub static EQ7_INFO: PluginInfo = PluginInfo {
        id: "eq_7band",
        name: "EQ 7-Band",
        short_name: "EQ7",
        category: Category::Eq,
        params: &EQ7_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
    const EQ1_TYPES: &[&str] = &["Peak", "Low Shelf", "High Shelf", "High Pass", "Low Pass", "Notch", "Band Pass"];
    static EQ1_PARAMS: [ParamInfo; 5] = [
        choice("type", "Type", EQ1_TYPES, 0),
        log("freq", "Freq", 20.0, 20000.0, 1000.0, Unit::Hz),
        lin("gain", "Gain", -24.0, 24.0, 0.0, Unit::Db),
        log("q", "Q", 0.1, 10.0, 1.0, Unit::None),
        lin("output_gain", "Output", -24.0, 24.0, 0.0, Unit::Db),
    ];
    pub static EQ1_INFO: PluginInfo = PluginInfo {
        id: "eq_1band",
        name: "EQ 1-Band",
        short_name: "EQ1",
        category: Category::Eq,
        params: &EQ1_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
}
pub mod harmonic {
    #[allow(unused_imports)]
    use crate::params::{choice, lin, log, toggle};
    #[allow(unused_imports)]
    use crate::{Category, ParamInfo, PluginInfo, Unit};
    static SAT_PARAMS: [ParamInfo; 5] = [
        lin("drive", "Drive", 0.0, 36.0, 6.0, Unit::Db),
        lin("bias", "Asymmetry", 0.0, 100.0, 0.0, Unit::Percent),
        log("tone", "Tone", 1000.0, 20000.0, 20000.0, Unit::Hz),
        lin("mix", "Mix", 0.0, 100.0, 100.0, Unit::Percent),
        lin("output", "Output", -24.0, 12.0, 0.0, Unit::Db),
    ];
    pub static SAT_INFO: PluginInfo = PluginInfo {
        id: "saturator",
        name: "Saturator",
        short_name: "Satur",
        category: Category::Harmonic,
        params: &SAT_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
    static LOFI_PARAMS: [ParamInfo; 4] = [
        lin("bits", "Bit Depth", 1.0, 24.0, 8.0, Unit::None),
        log("sample_rate", "Sample Rate", 500.0, 48000.0, 11025.0, Unit::Hz),
        lin("noise", "Noise", 0.0, 100.0, 0.0, Unit::Percent),
        lin("mix", "Mix", 0.0, 100.0, 100.0, Unit::Percent),
    ];
    pub static LOFI_INFO: PluginInfo = PluginInfo {
        id: "lofi",
        name: "Lo-Fi",
        short_name: "LoFi",
        category: Category::Harmonic,
        params: &LOFI_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
    static RECT_PARAMS: [ParamInfo; 3] = [
        choice("mode", "Mode", &["Half Wave", "Full Wave"], 1),
        lin("mix", "Mix", 0.0, 100.0, 100.0, Unit::Percent),
        lin("output", "Output", -24.0, 12.0, 0.0, Unit::Db),
    ];
    pub static RECT_INFO: PluginInfo = PluginInfo {
        id: "rectifier",
        name: "Rectifier",
        short_name: "Rectify",
        category: Category::Harmonic,
        params: &RECT_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
}
pub mod modulation {
    #[allow(unused_imports)]
    use crate::params::{choice, lin, log, toggle};
    #[allow(unused_imports)]
    use crate::{Category, ParamInfo, PluginInfo, Unit};
    const MAX_DELAY_MS: f32 = 2000.0;
    const MOD_DELAY_DEPTH_MS: f32 = 5.0;
    static DELAY_PARAMS: [ParamInfo; 6] = [
        log("time", "Time", 1.0, MAX_DELAY_MS, 250.0, Unit::Ms),
        lin("feedback", "Feedback", -99.0, 99.0, 30.0, Unit::Percent),
        lin("mix", "Mix", 0.0, 100.0, 30.0, Unit::Percent),
        log("mod_rate", "Mod Rate", 0.01, 10.0, 0.5, Unit::Hz),
        lin("mod_depth", "Mod Depth", 0.0, 100.0, 0.0, Unit::Percent),
        log("lpf", "LPF", 500.0, 20000.0, 12000.0, Unit::Hz),
    ];
    pub static DELAY_INFO: PluginInfo = PluginInfo {
        id: "mod_delay",
        name: "Mod Delay",
        short_name: "ModDly",
        category: Category::Delay,
        params: &DELAY_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
    static CHORUS_PARAMS: [ParamInfo; 6] = [
        log("rate", "Rate", 0.05, 5.0, 0.8, Unit::Hz),
        lin("depth", "Depth", 0.0, 100.0, 50.0, Unit::Percent),
        lin("delay", "Delay", 5.0, 30.0, 12.0, Unit::Ms),
        lin("feedback", "Feedback", 0.0, 90.0, 0.0, Unit::Percent),
        lin("mix", "Mix", 0.0, 100.0, 50.0, Unit::Percent),
        lin("spread", "Spread", 0.0, 100.0, 100.0, Unit::Percent),
    ];
    pub static CHORUS_INFO: PluginInfo = PluginInfo {
        id: "chorus",
        name: "Chorus",
        short_name: "Chorus",
        category: Category::Modulation,
        params: &CHORUS_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
    static FLANGER_PARAMS: [ParamInfo; 5] = [
        log("rate", "Rate", 0.02, 5.0, 0.25, Unit::Hz),
        lin("depth", "Depth", 0.0, 100.0, 70.0, Unit::Percent),
        lin("delay", "Delay", 0.1, 10.0, 2.0, Unit::Ms),
        lin("feedback", "Feedback", -95.0, 95.0, 50.0, Unit::Percent),
        lin("mix", "Mix", 0.0, 100.0, 50.0, Unit::Percent),
    ];
    pub static FLANGER_INFO: PluginInfo = PluginInfo {
        id: "flanger",
        name: "Flanger",
        short_name: "Flanger",
        category: Category::Modulation,
        params: &FLANGER_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
    const STAGE_CHOICES: &[&str] = &["2", "4", "6", "8", "12"];
    const MAX_STAGES: usize = 12;
    static PHASER_PARAMS: [ParamInfo; 6] = [
        log("rate", "Rate", 0.02, 5.0, 0.5, Unit::Hz),
        lin("depth", "Depth", 0.0, 100.0, 70.0, Unit::Percent),
        choice("stages", "Stages", STAGE_CHOICES, 2),
        lin("feedback", "Feedback", -95.0, 95.0, 40.0, Unit::Percent),
        log("center", "Center", 100.0, 4000.0, 800.0, Unit::Hz),
        lin("mix", "Mix", 0.0, 100.0, 50.0, Unit::Percent),
    ];
    pub static PHASER_INFO: PluginInfo = PluginInfo {
        id: "phaser",
        name: "Phaser",
        short_name: "Phaser",
        category: Category::Modulation,
        params: &PHASER_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
}
pub mod pitch {
    #[allow(unused_imports)]
    use crate::params::{choice, lin, log, toggle};
    #[allow(unused_imports)]
    use crate::{Category, ParamInfo, PluginInfo, Unit};
    const WINDOW_MS: f32 = 50.0;
    static PITCH_PARAMS: [ParamInfo; 3] = [
        lin("semitones", "Semitones", -24.0, 24.0, 0.0, Unit::Semitones),
        lin("cents", "Fine", -100.0, 100.0, 0.0, Unit::Cents),
        lin("mix", "Mix", 0.0, 100.0, 100.0, Unit::Percent),
    ];
    pub static PITCH_INFO: PluginInfo = PluginInfo {
        id: "pitch_shifter",
        name: "Pitch Shifter",
        short_name: "Pitch",
        category: Category::PitchShift,
        params: &PITCH_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
}
pub mod reverb {
    macro_rules! reverb_params {
        ($mix:expr, $pre:expr, $decay:expr, $damp:expr, $diff:expr) => {
            [
                lin("mix", "Mix", 0.0, 100.0, $mix, Unit::Percent),
                lin("pre_delay", "Pre-Delay", 0.0, MAX_PREDELAY_MS, $pre, Unit::Ms),
                log("decay", "Decay", 0.1, 20.0, $decay, Unit::Seconds),
                lin("size", "Size", 0.0, 100.0, 50.0, Unit::Percent),
                log("damping", "HF Damping", 1000.0, 20000.0, $damp, Unit::Hz),
                lin("diffusion", "Diffusion", 0.0, 100.0, $diff, Unit::Percent),
                lin("width", "Width", 0.0, 100.0, 100.0, Unit::Percent),
                log("low_cut", "Low Cut", 20.0, 1000.0, 20.0, Unit::Hz),
            ]
        };
    }
    #[allow(unused_imports)]
    use crate::params::{choice, lin, log, toggle};
    #[allow(unused_imports)]
    use crate::{Category, ParamInfo, PluginInfo, Unit};
    const LINES: usize = 8;
    const DIFFUSERS: usize = 4;
    const MAX_PREDELAY_MS: f32 = 250.0;
    const SIZE_MIN: f32 = 0.35;
    const SIZE_MAX: f32 = 1.6;
    static ROOM_PARAMS: [ParamInfo; 8] = reverb_params!(25.0, 10.0, 1.2, 6000.0, 70.0);
    static PLATE_PARAMS: [ParamInfo; 8] = reverb_params!(30.0, 20.0, 2.5, 10000.0, 85.0);
    pub static ROOM_INFO: PluginInfo = PluginInfo {
        id: "room_reverb",
        name: "Room Reverb",
        short_name: "Room",
        category: Category::Reverb,
        params: &ROOM_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
    pub static PLATE_INFO: PluginInfo = PluginInfo {
        id: "plate_reverb",
        name: "Plate Reverb",
        short_name: "Plate",
        category: Category::Reverb,
        params: &PLATE_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
}
pub mod strip {
    #[allow(unused_imports)]
    use crate::params::{choice, lin, log, toggle};
    #[allow(unused_imports)]
    use crate::{Category, ParamInfo, PluginInfo, Unit};
    static STRIP_PARAMS: [ParamInfo; 22] = [
        lin("input_gain", "Input", -24.0, 24.0, 0.0, Unit::Db),
        toggle("gate_on", "Gate", false),
        lin("gate_threshold", "Gate Thresh", -80.0, 0.0, -50.0, Unit::Db),
        lin("gate_range", "Gate Range", 0.0, 80.0, 40.0, Unit::Db),
        log("gate_release", "Gate Release", 1.0, 4000.0, 150.0, Unit::Ms),
        toggle("comp_on", "Comp", false),
        lin("comp_threshold", "Comp Thresh", -60.0, 0.0, -18.0, Unit::Db),
        log("comp_ratio", "Comp Ratio", 1.0, 100.0, 3.0, Unit::Ratio),
        log("comp_attack", "Comp Attack", 0.01, 300.0, 10.0, Unit::Ms),
        log("comp_release", "Comp Release", 5.0, 3000.0, 120.0, Unit::Ms),
        lin("comp_makeup", "Comp Makeup", 0.0, 40.0, 0.0, Unit::Db),
        toggle("eq_on", "EQ", true),
        toggle("hpf_on", "HPF", false),
        log("hpf_freq", "HPF Freq", 10.0, 2000.0, 80.0, Unit::Hz),
        lin("low_gain", "Low Gain", -24.0, 24.0, 0.0, Unit::Db),
        log("low_freq", "Low Freq", 20.0, 1000.0, 100.0, Unit::Hz),
        lin("mid_gain", "Mid Gain", -24.0, 24.0, 0.0, Unit::Db),
        log("mid_freq", "Mid Freq", 100.0, 8000.0, 1000.0, Unit::Hz),
        log("mid_q", "Mid Q", 0.1, 10.0, 1.0, Unit::None),
        lin("high_gain", "High Gain", -24.0, 24.0, 0.0, Unit::Db),
        log("high_freq", "High Freq", 1000.0, 20000.0, 8000.0, Unit::Hz),
        lin("output_gain", "Output", -24.0, 24.0, 0.0, Unit::Db),
    ];
    pub static STRIP_INFO: PluginInfo = PluginInfo {
        id: "channel_strip",
        name: "Channel Strip",
        short_name: "Strip",
        category: Category::Dynamics,
        params: &STRIP_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
}
pub mod synth {
    #[allow(unused_imports)]
    use crate::params::{choice, lin, log, toggle};
    #[allow(unused_imports)]
    use crate::{Category, ParamInfo, PluginInfo, Unit};
    const WAVES: &[&str] = &["Saw", "Square", "Sine", "Triangle"];
    static SYNTH_PARAMS: [ParamInfo; 19] = [
        choice("osc1_wave", "Osc 1", WAVES, 0),
        choice("osc2_wave", "Osc 2", WAVES, 0),
        lin("osc2_semitones", "Osc 2 Pitch", -24.0, 24.0, 0.0, Unit::Semitones),
        lin("osc2_detune", "Osc 2 Detune", -100.0, 100.0, 7.0, Unit::Cents),
        lin("osc_mix", "Osc Mix", 0.0, 100.0, 50.0, Unit::Percent),
        log("cutoff", "Cutoff", 20.0, 20000.0, 2000.0, Unit::Hz),
        lin("resonance", "Resonance", 0.0, 100.0, 20.0, Unit::Percent),
        lin("filter_env_amount", "Filter Env", -100.0, 100.0, 30.0, Unit::Percent),
        log("amp_attack", "Amp Attack", 0.5, 5000.0, 5.0, Unit::Ms),
        log("amp_decay", "Amp Decay", 1.0, 5000.0, 200.0, Unit::Ms),
        lin("amp_sustain", "Amp Sustain", 0.0, 100.0, 70.0, Unit::Percent),
        log("amp_release", "Amp Release", 1.0, 10000.0, 300.0, Unit::Ms),
        log("filter_attack", "Filter Attack", 0.5, 5000.0, 5.0, Unit::Ms),
        log("filter_decay", "Filter Decay", 1.0, 5000.0, 300.0, Unit::Ms),
        lin("filter_sustain", "Filter Sustain", 0.0, 100.0, 30.0, Unit::Percent),
        log("filter_release", "Filter Release", 1.0, 10000.0, 300.0, Unit::Ms),
        lin("glide", "Glide", 0.0, 1000.0, 0.0, Unit::Ms),
        lin("velocity", "Velocity Sens", 0.0, 100.0, 70.0, Unit::Percent),
        lin("level", "Level", -48.0, 6.0, -6.0, Unit::Db),
    ];
    pub static SYNTH_INFO: PluginInfo = PluginInfo {
        id: "subtractive_synth",
        name: "Subtractive Synth",
        short_name: "SubSynth",
        category: Category::Instrument,
        params: &SYNTH_PARAMS,
        is_instrument: true,
        audiosuite: false,
    };
    const VOICES: usize = 16;
    const SILENT: f32 = 1.0e-5;
}
pub mod utility {
    #[allow(unused_imports)]
    use crate::params::{choice, lin, log, toggle};
    #[allow(unused_imports)]
    use crate::{Category, ParamInfo, PluginInfo, Unit};
    const OFF_DB: f32 = -96.0;
    const MAX_SHIFT_MS: f32 = 1000.0;
    const MAX_SHIFT_SAMPLES: f32 = 10_000.0;
    const XFADE: usize = 256;
    static SHIFT_PARAMS: [ParamInfo; 2] =
        [lin("delay_ms", "Delay", 0.0, MAX_SHIFT_MS, 0.0, Unit::Ms), lin("delay_samples", "Samples", 0.0, MAX_SHIFT_SAMPLES, 0.0, Unit::None)];
    pub static SHIFT_INFO: PluginInfo = PluginInfo {
        id: "time_shift",
        name: "Time Shift",
        short_name: "TShift",
        category: Category::Other,
        params: &SHIFT_PARAMS,
        is_instrument: false,
        audiosuite: false,
    };
    static GAIN_PARAMS: [ParamInfo; 1] = [lin("gain", "Gain", OFF_DB, 24.0, 0.0, Unit::Db)];
    pub static GAIN_INFO: PluginInfo = PluginInfo {
        id: "gain",
        name: "Gain",
        short_name: "Gain",
        category: Category::Other,
        params: &GAIN_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
    static TRIM_PARAMS: [ParamInfo; 2] = [lin("gain", "Trim", OFF_DB, 12.0, 0.0, Unit::Db), toggle("invert", "Phase Invert", false)];
    pub static TRIM_INFO: PluginInfo = PluginInfo {
        id: "trim",
        name: "Trim",
        short_name: "Trim",
        category: Category::Other,
        params: &TRIM_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
    pub static INVERT_INFO: PluginInfo = PluginInfo {
        id: "invert",
        name: "Invert",
        short_name: "Invert",
        category: Category::Other,
        params: &[],
        is_instrument: false,
        audiosuite: true,
    };
    static DC_PARAMS: [ParamInfo; 1] = [log("cutoff", "Cutoff", 1.0, 40.0, 5.0, Unit::Hz)];
    pub static DC_INFO: PluginInfo = PluginInfo {
        id: "dc_offset_removal",
        name: "DC Offset Removal",
        short_name: "DC Rmv",
        category: Category::Other,
        params: &DC_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
    const WAVE_NAMES: &[&str] = &["Sine", "Square", "Saw", "Triangle", "White Noise", "Pink Noise"];
    static GEN_PARAMS: [ParamInfo; 3] = [
        choice("waveform", "Waveform", WAVE_NAMES, 0),
        log("freq", "Frequency", 20.0, 20000.0, 1000.0, Unit::Hz),
        lin("level", "Level", OFF_DB, 0.0, -20.0, Unit::Db),
    ];
    pub static GEN_INFO: PluginInfo = PluginInfo {
        id: "signal_generator",
        name: "Signal Generator",
        short_name: "SigGen",
        category: Category::Other,
        params: &GEN_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
    static DITHER_PARAMS: [ParamInfo; 2] = [choice("bits", "Bit Depth", &["16", "20", "24"], 0), toggle("noise_shaping", "Noise Shaping", false)];
    pub static DITHER_INFO: PluginInfo = PluginInfo {
        id: "dither",
        name: "Dither",
        short_name: "Dither",
        category: Category::Dither,
        params: &DITHER_PARAMS,
        is_instrument: false,
        audiosuite: true,
    };
}
