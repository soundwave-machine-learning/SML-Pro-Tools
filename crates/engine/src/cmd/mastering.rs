//! Mastering workspace commands (R34): program source, monitor-only reference with level
//! matching for audition, standard meters over the program (offline analysis of the current
//! document), and the status the Master layout shows. The chain is the Master track's user
//! inserts (processed exactly once: the export path is the main mix). The reference never
//! reaches a bounce: it lives outside the document's audio graph (playback monitor path only).

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_model::{Reference, SourceId, TrackKind};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "mastering.program",
            "Mastering Program Source",
            [],
            None,
            "{source?: id|null} — null = the session's mix; an imported stereo source plays through the master chain instead",
            always,
            |e, p| {
                let src = match p.get("source") {
                    None | Some(Value::Null) => None,
                    Some(v) => {
                        let id = SourceId(v.as_u64().ok_or_else(|| bad("mastering.program", "`source` must be a source id or null"))?);
                        if e.session().pool.get(id).is_none() {
                            return Err(bad("mastering.program", format!("source {} is not loaded", id.0)));
                        }
                        Some(id)
                    }
                };
                e.session_mut().mastering.program = src;
                Ok(json!({"program": src.map(|s| s.0)}))
            }
        ),
        cmd!(
            "mastering.reference",
            "Reference Track",
            [],
            None,
            "{source?: id|null, listening?: bool, gain_db?: -60..12} — monitor-only, bypasses the master chain, never exported",
            always,
            |e, p| {
                let s = e.session_mut();
                match p.get("source") {
                    Some(Value::Null) => s.mastering.reference = None,
                    Some(v) => {
                        let id = SourceId(v.as_u64().ok_or_else(|| bad("mastering.reference", "`source` must be a source id or null"))?);
                        if s.pool.get(id).is_none() {
                            return Err(bad("mastering.reference", format!("source {} is not loaded", id.0)));
                        }
                        let prev = s.mastering.reference.take();
                        let (gain_db, listening) = prev.map_or((0.0, false), |r| (r.gain_db, r.listening));
                        s.mastering.reference = Some(Reference { source: id, gain_db, listening });
                    }
                    None => {}
                }
                if let Some(r) = s.mastering.reference.as_mut() {
                    if let Some(l) = p.get("listening").and_then(Value::as_bool) {
                        r.listening = l;
                    }
                    if let Some(g) = p.get("gain_db").and_then(Value::as_f64).filter(|g| g.is_finite()) {
                        r.gain_db = (g as f32).clamp(-60.0, 12.0);
                    }
                } else if p.get("listening").is_some() || p.get("gain_db").is_some() {
                    return Err(bad("mastering.reference", "no reference source set"));
                }
                Ok(json!(s.mastering.reference))
            }
        ),
        cmd!(query "mastering.level_match", "Level-Match Reference", [], None, "{} → {program_lufs, reference_lufs, gain_db} — audition gain for the reference so both sit at the program's integrated loudness; never an export setting", always, |e, _| {
            let s = e.session();
            let r = s.mastering.reference.as_ref().ok_or_else(|| bad("mastering.level_match", "no reference source set"))?;
            let range = Range::new(0, s.content_end().max(1));
            let prog = analyse(s, range);
            let refr = s.pool.get(r.source).ok_or_else(|| bad("mastering.level_match", "reference not loaded"))?;
            let mut m = soundcraft_dsp::meter::LoudnessMeter::new(s.sample_rate.as_f64() as f32, refr.buffer.channels.len());
            m.process(&refr.buffer.channels, refr.buffer.channels.first().map_or(0, Vec::len));
            let gain = (prog.integrated_lufs - m.integrated_lufs()).clamp(-60.0, 12.0);
            Ok(json!({"program_lufs": prog.integrated_lufs, "reference_lufs": m.integrated_lufs(), "gain_db": gain, "note": "audition only; apply with mastering.reference {gain_db}"}))
        }),
        cmd!(query "mastering.meters", "Mastering Meters", [], None, "{start?, end?} → sample peak, true peak (4×, BS.1770-4), momentary/short-term max, integrated (gated −70 abs / −10 rel), loudness range (EBU Tech 3342), correlation, mono-sum peak — offline over the program", always, |e, p| {
            let s = e.session();
            let r = range_param(e, "mastering.meters", p)?;
            let r = if r.is_empty() { Range::new(0, s.content_end().max(1)) } else { r };
            Ok(json!(analyse(s, r)))
        }),
        cmd!(query "mastering.status", "Mastering Status", [], None, "{} → {program, chain: [inserts of the Master track], reference, export_exclusions, standards}", always, |e, _| {
            let s = e.session();
            let chain: Vec<Value> = s.tracks.iter().filter(|t| t.kind == TrackKind::Master).flat_map(|t| t.mixer.inserts.iter().enumerate().filter_map(|(i, x)| x.as_ref().map(|x| json!({"slot": i, "plugin": x.plugin, "bypass": x.bypass, "version": x.plugin_version})))).collect();
            Ok(json!({
                "program": s.mastering.program.map(|x| x.0),
                "chain": chain,
                "chain_applied": "once, on the Master track (the export path is the main mix)",
                "reference": s.mastering.reference,
                "export_exclusions": ["reference", "monitor volume/dim/mono", "cue", "assistant speech"],
                "standards": {"loudness": "ITU-R BS.1770-4 / EBU R128 (K-weighting; absolute gate −70 LUFS; relative gate −10 LU)", "true_peak": "4× oversampled (BS.1770-4 Annex 2)", "loudness_range": "EBU Tech 3342 (short-term 3 s, −70 abs / −20 rel, 10th–95th percentile)", "provenance": "in-house implementation checked against analytic Tech 3341/3342 signals (E07); not a certification"},
            }))
        }),
    ]
}

/// Standard meters over `range` of the program (the main mix, or the imported program source).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Meters {
    pub range: (i64, i64),
    pub sample_peak_db: f32,
    pub true_peak_db: f32,
    pub max_momentary_lufs: f32,
    pub max_short_term_lufs: f32,
    pub integrated_lufs: f32,
    pub loudness_range_lu: f32,
    pub correlation: f32,
    pub mono_peak_db: f32,
}

pub fn analyse(s: &soundcraft_model::Session, range: Range) -> Meters {
    let sr = s.sample_rate.as_f64() as f32;
    let audio: Vec<Vec<f32>> = match s.mastering.program.and_then(|id| s.pool.get(id)) {
        Some(src) => {
            let a = usize::try_from(range.start.max(0)).unwrap_or(0);
            let b = usize::try_from(range.end.max(0)).unwrap_or(0);
            src.buffer.channels.iter().map(|c| c.get(a.min(c.len())..b.min(c.len())).map(<[f32]>::to_vec).unwrap_or_default()).collect()
        }
        None => soundcraft_mix::render_range(s, range, 1024),
    };
    let frames = audio.first().map_or(0, Vec::len);
    let mut m = soundcraft_dsp::meter::LoudnessMeter::new(sr, audio.len().max(1));
    let mut max_st = -144.0f32;
    // Feed in 100 ms steps so the short-term maximum is tracked.
    let step = (sr * 0.1) as usize;
    let mut pos = 0;
    while pos < frames {
        let n = step.min(frames - pos);
        let chunk: Vec<Vec<f32>> = audio.iter().map(|c| c[pos..pos + n].to_vec()).collect();
        m.process(&chunk, n);
        max_st = max_st.max(m.short_term_lufs());
        pos += n;
    }
    let (mut dot, mut l2, mut r2, mut mono_peak) = (0.0f64, 0.0f64, 0.0f64, 0.0f32);
    if audio.len() >= 2 {
        for (l, r) in audio[0].iter().zip(&audio[1]) {
            dot += f64::from(l * r);
            l2 += f64::from(l * l);
            r2 += f64::from(r * r);
            mono_peak = mono_peak.max(((l + r) * 0.5).abs());
        }
    } else if let Some(c) = audio.first() {
        mono_peak = c.iter().fold(0.0f32, |a, x| a.max(x.abs()));
    }
    let correlation = if l2 > 0.0 && r2 > 0.0 { (dot / (l2 * r2).sqrt()) as f32 } else { 0.0 };
    Meters {
        range: (range.start, range.end),
        sample_peak_db: soundcraft_dsp::offline::peak_db(&audio),
        true_peak_db: m.true_peak_db(),
        max_momentary_lufs: m.max_momentary_lufs(),
        max_short_term_lufs: max_st,
        integrated_lufs: m.integrated_lufs(),
        loudness_range_lu: m.loudness_range_lu(),
        correlation,
        mono_peak_db: 20.0 * mono_peak.max(1e-7).log10(),
    }
}
