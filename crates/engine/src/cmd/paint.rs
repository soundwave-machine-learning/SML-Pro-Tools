//! Paint tool shapes (R11 / T06): `automation.draw` writes a numerically defined curve into a
//! track automation lane over a range, replacing the points inside it and keeping everything
//! outside; one undo step. Shapes, conventions and bounds (declared here before the tests):
//!
//! - Horizontal axis `u` runs 0 → 1 from `start` to `end` (samples); the generated points are
//!   spaced `density_ms` apart (default 10 ms, minimum 1 ms, at most 100 000 points) plus the
//!   exact endpoints, so the lane's linear interpolation reproduces the curve within the
//!   declared density.
//! - `line`: `from + (to - from) · u`.
//! - `parabola`: `from + (to - from) · u²` (convex; `curve: -1` gives the concave `1-(1-u)²`).
//! - Periodic shapes (`square`, `triangle`, `saw`, `sine`): `baseline + amplitude · f(phase_u)`
//!   with `phase_u = cycles · u + phase` (cycles default 1, phase in cycles, default 0), f in
//!   [-1, 1]: square = +1 for the first half of each cycle, −1 for the second; triangle rises
//!   0→1 over the first half and falls 1→0 over the second (f = 2·tri − 1, so it spans −1..1);
//!   saw rises −1→1 across each cycle; sine = sin(2π · phase_u).
//! - `freehand`: the caller's `(at, value)` points inside the range, in time order.
//! - `transform`: shifts/scales the existing points inside `[box_start, box_end]`:
//!   `at' = box_start + (at − box_start) · time_scale + time_shift`,
//!   `value' = value · value_scale + value_offset`; points leaving the box are dropped, results
//!   are clamped to the parameter range; non-finite inputs are rejected before any change.
//!
//! Values are clamped to `AutoParam::range`. Clip-gain *curves* are not a model entity in this
//! build (clip gain is a static value plus fades), so Paint targets automation lanes only; that
//! limitation is stated in the user guide. Drawing never touches the audio callback: the lane
//! is replaced in the document, and the engine reads it on the next block.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_model::AutoParam;

/// Default and minimum point spacing.
pub const DEFAULT_DENSITY_MS: f64 = 10.0;
pub const MIN_DENSITY_MS: f64 = 1.0;
pub const MAX_POINTS: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Freehand,
    Line,
    Parabola,
    Square,
    Triangle,
    Saw,
    Sine,
    Transform,
}

impl Shape {
    pub fn parse(s: &str) -> Option<Shape> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "freehand" => Shape::Freehand,
            "line" => Shape::Line,
            "parabola" => Shape::Parabola,
            "square" => Shape::Square,
            "triangle" => Shape::Triangle,
            "saw" | "sawtooth" => Shape::Saw,
            "sine" => Shape::Sine,
            "transform" => Shape::Transform,
            _ => return None,
        })
    }
    pub const ALL: [&'static str; 8] = ["freehand", "line", "parabola", "square", "triangle", "saw", "sine", "transform"];
    pub fn id(self) -> &'static str {
        match self {
            Shape::Freehand => "freehand",
            Shape::Line => "line",
            Shape::Parabola => "parabola",
            Shape::Square => "square",
            Shape::Triangle => "triangle",
            Shape::Saw => "saw",
            Shape::Sine => "sine",
            Shape::Transform => "transform",
        }
    }
}

/// Parameters of one shape (values in the parameter's own units).
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeParams {
    pub from: f64,
    pub to: f64,
    pub curve: f64,
    pub baseline: f64,
    pub amplitude: f64,
    pub cycles: f64,
    pub phase: f64,
    pub density_ms: f64,
}

/// Periodic shape value in [-1, 1] at `phase_u` (cycles).
fn periodic(shape: Shape, phase_u: f64) -> f64 {
    let frac = phase_u.rem_euclid(1.0);
    match shape {
        Shape::Square => {
            if frac < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Shape::Triangle => {
            let tri = if frac < 0.5 { frac * 2.0 } else { 2.0 - frac * 2.0 };
            tri * 2.0 - 1.0
        }
        Shape::Saw => frac * 2.0 - 1.0,
        Shape::Sine => (std::f64::consts::TAU * phase_u).sin(),
        _ => 0.0,
    }
}

/// The points of `shape` over `[start, end]` at the sample rate `sr` (not for freehand /
/// transform). Values are not yet clamped.
pub fn shape_points(shape: Shape, start: Samples, end: Samples, sr: f64, p: &ShapeParams) -> Vec<(Samples, f64)> {
    let len = (end - start).max(1) as f64;
    let step = (p.density_ms.max(MIN_DENSITY_MS) / 1000.0 * sr).max(1.0);
    let n = ((len / step).ceil() as usize).clamp(1, MAX_POINTS);
    let mut out = Vec::with_capacity(n + 1);
    for k in 0..=n {
        let at = if k == n { end } else { start + ((k as f64) * step) as Samples };
        if k > 0 && at <= out.last().map_or(start - 1, |x: &(Samples, f64)| x.0) {
            continue;
        }
        let u = ((at - start) as f64 / len).clamp(0.0, 1.0);
        let v = match shape {
            Shape::Line => p.from + (p.to - p.from) * u,
            Shape::Parabola => {
                let w = if p.curve < 0.0 { 1.0 - (1.0 - u) * (1.0 - u) } else { u * u };
                p.from + (p.to - p.from) * w
            }
            Shape::Square | Shape::Triangle | Shape::Saw | Shape::Sine => p.baseline + p.amplitude * periodic(shape, p.cycles * u + p.phase),
            Shape::Freehand | Shape::Transform => p.from,
        };
        out.push((at, v));
    }
    out
}

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "automation.draw",
        "Draw Automation Shape",
        [],
        None,
        "{tracks?, param, shape: freehand|line|parabola|square|triangle|saw|sine|transform, start?, end?, from?, to?, curve?, baseline?, amplitude?, cycles?: 1, phase?: 0, density_ms?: 10, points?: [[at, value]…] (freehand), box_start?, box_end?, time_shift?, time_scale?: 1, value_offset?, value_scale?: 1 (transform)}",
        has_selection,
        draw
    )]
}

fn finite(p: &Value, key: &str, d: f64) -> Result<f64> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(d),
        Some(v) => v.as_f64().filter(|x| x.is_finite()).ok_or_else(|| bad("automation.draw", format!("`{key}` must be a finite number"))),
    }
}

fn draw(e: &mut Engine, p: &Value) -> Result<Value> {
    let cmd = "automation.draw";
    let tracks = tracks_required(e, cmd, p)?;
    let param = str_param(p, "param").and_then(AutoParam::parse).ok_or_else(|| bad(cmd, "`param` required (volume, pan, send:N, …)"))?;
    let shape = str_param(p, "shape").and_then(Shape::parse).ok_or_else(|| bad(cmd, format!("`shape` must be one of {}", Shape::ALL.join("|"))))?;
    let r = range_param(e, cmd, p)?;
    if r.is_empty() && shape != Shape::Transform {
        return Err(bad(cmd, "a non-empty range (start < end) is required"));
    }
    let (lo, hi, default) = param.range();
    let sr = e.session().sample_rate.as_f64();
    let sp = ShapeParams {
        from: finite(p, "from", f64::from(default))?,
        to: finite(p, "to", f64::from(default))?,
        curve: finite(p, "curve", 1.0)?,
        baseline: finite(p, "baseline", f64::from(default))?,
        amplitude: finite(p, "amplitude", f64::from((hi - lo) / 2.0))?,
        cycles: finite(p, "cycles", 1.0)?,
        phase: finite(p, "phase", 0.0)?,
        density_ms: finite(p, "density_ms", DEFAULT_DENSITY_MS)?,
    };
    if sp.cycles <= 0.0 || sp.cycles > 1_000_000.0 {
        return Err(bad(cmd, "`cycles` must be positive"));
    }
    let clamp = |v: f64| (v as f32).clamp(lo, hi);
    let mut written = 0usize;
    match shape {
        Shape::Transform => {
            let box_start = position_param(e, cmd, p, "box_start")?.unwrap_or(r.start);
            let box_end = position_param(e, cmd, p, "box_end")?.unwrap_or(r.end);
            if box_end <= box_start {
                return Err(bad(cmd, "`box_end` must be after `box_start`"));
            }
            let time_shift = finite(p, "time_shift", 0.0)?;
            let time_scale = finite(p, "time_scale", 1.0)?;
            let value_offset = finite(p, "value_offset", 0.0)?;
            let value_scale = finite(p, "value_scale", 1.0)?;
            if time_scale <= 0.0 || time_scale > 1000.0 {
                return Err(bad(cmd, "`time_scale` must be in (0, 1000]"));
            }
            let s = e.session_mut();
            for t in &tracks {
                let Some(tr) = s.track_mut(*t) else { continue };
                let lane = tr.lane_mut(&param);
                let inside: Vec<(Samples, f32)> =
                    lane.points.iter().filter(|q| q.at >= box_start && q.at <= box_end).map(|q| (q.at, q.value)).collect();
                // The box is inclusive at both ends; `clear_range` is end-exclusive.
                lane.points.retain(|q| q.at < box_start || q.at > box_end);
                for (at, v) in inside {
                    let nat = box_start + (((at - box_start) as f64) * time_scale + time_shift).round() as Samples;
                    if nat < box_start || nat > box_end {
                        continue;
                    }
                    lane.set_point(nat, clamp(f64::from(v) * value_scale + value_offset));
                    written += 1;
                }
            }
        }
        Shape::Freehand => {
            let pts: Vec<(Samples, f64)> = p
                .get("points")
                .and_then(Value::as_array)
                .ok_or_else(|| bad(cmd, "`points` ([[at, value]…]) required for freehand"))?
                .iter()
                .map(|q| {
                    let at = q.get(0).and_then(Value::as_i64).ok_or_else(|| bad(cmd, "point `at` must be an integer sample"))?;
                    let v = q.get(1).and_then(Value::as_f64).filter(|x| x.is_finite()).ok_or_else(|| bad(cmd, "point value must be finite"))?;
                    Ok((at, v))
                })
                .collect::<Result<Vec<_>>>()?;
            if pts.len() > MAX_POINTS {
                return Err(bad(cmd, format!("at most {MAX_POINTS} points")));
            }
            let s = e.session_mut();
            for t in &tracks {
                let Some(tr) = s.track_mut(*t) else { continue };
                let lane = tr.lane_mut(&param);
                lane.clear_range(r.start, r.end);
                for (at, v) in pts.iter().filter(|(at, _)| *at >= r.start && *at <= r.end) {
                    lane.set_point(*at, clamp(*v));
                    written += 1;
                }
            }
        }
        _ => {
            let pts = shape_points(shape, r.start, r.end, sr, &sp);
            let s = e.session_mut();
            for t in &tracks {
                let Some(tr) = s.track_mut(*t) else { continue };
                let lane = tr.lane_mut(&param);
                lane.clear_range(r.start, r.end);
                for (at, v) in &pts {
                    lane.set_point(*at, clamp(*v));
                    written += 1;
                }
            }
        }
    }
    Ok(json!({"tracks": tracks.len(), "points": written, "start": r.start, "end": r.end}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_model::{ChannelFormat, TrackKind};

    fn engine() -> (Engine, soundcraft_model::TrackId) {
        let mut e = Engine::default();
        let t = e.session_mut().add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Vox"));
        (e, t)
    }

    fn value(e: &Engine, t: soundcraft_model::TrackId, at: Samples) -> f32 {
        e.session().track(t).unwrap().lane(&AutoParam::Volume).unwrap().value_at(at, 0.0)
    }

    /// T-T2: every shape produces its declared curve (checked at analytic points), the range
    /// outside the drawn region is untouched, values clamp to the parameter range, and one
    /// draw is one undo step.
    #[test]
    fn shapes_match_their_declared_curves_and_keep_outside_points() {
        let (mut e, t) = engine();
        // Existing points outside the range must survive.
        e.execute("automation.set_point", &json!({"tracks": [t.0], "param": "volume", "at": 0, "value": -3.0})).unwrap();
        e.execute("automation.set_point", &json!({"tracks": [t.0], "param": "volume", "at": 200_000, "value": -9.0})).unwrap();
        let (s0, s1) = (48_000i64, 96_000i64);
        let depth = e.undo_depth();
        let r = e
            .execute(
                "automation.draw",
                &json!({"tracks": [t.0], "param": "volume", "shape": "line", "start": s0, "end": s1, "from": -12.0, "to": 0.0}),
            )
            .unwrap();
        assert!(r["points"].as_u64().unwrap() >= 100);
        assert_eq!(e.undo_depth(), depth + 1, "one undo step");
        assert!((value(&e, t, s0) + 12.0).abs() < 1e-4);
        assert!((value(&e, t, (s0 + s1) / 2) + 6.0).abs() < 0.05, "midpoint of the line");
        assert!((value(&e, t, s1)).abs() < 1e-4);
        assert_eq!(value(&e, t, 0), -3.0, "outside point kept");
        assert_eq!(value(&e, t, 200_000), -9.0, "outside point kept");
        // Parabola: at u = 0.5 the value is from + 0.25 · (to − from).
        e.execute(
            "automation.draw",
            &json!({"tracks": [t.0], "param": "volume", "shape": "parabola", "start": s0, "end": s1, "from": -12.0, "to": 0.0}),
        )
        .unwrap();
        assert!((value(&e, t, (s0 + s1) / 2) + 9.0).abs() < 0.05, "{}", value(&e, t, (s0 + s1) / 2));
        // Sine, 2 cycles, amplitude 6 around −6: u = 0.125 → first quarter cycle peak = 0 dB.
        e.execute(
            "automation.draw",
            &json!({"tracks": [t.0], "param": "volume", "shape": "sine", "start": s0, "end": s1, "baseline": -6.0, "amplitude": 6.0, "cycles": 2}),
        )
        .unwrap();
        let at = s0 + (s1 - s0) / 8;
        assert!(value(&e, t, at).abs() < 0.1, "sine peak {}", value(&e, t, at));
        assert!((value(&e, t, s0) + 6.0).abs() < 1e-3, "sine starts at the baseline");
        // Square: first half of a cycle high, second half low.
        e.execute(
            "automation.draw",
            &json!({"tracks": [t.0], "param": "volume", "shape": "square", "start": s0, "end": s1, "baseline": -6.0, "amplitude": 6.0, "cycles": 1}),
        )
        .unwrap();
        assert!((value(&e, t, s0 + 1000)).abs() < 1e-3);
        assert!((value(&e, t, s0 + 30_000) + 12.0).abs() < 1e-3);
        // Triangle peaks mid-cycle; saw rises across the cycle.
        e.execute("automation.draw", &json!({"tracks": [t.0], "param": "volume", "shape": "triangle", "start": s0, "end": s1, "baseline": -6.0, "amplitude": 6.0, "cycles": 1})).unwrap();
        assert!(value(&e, t, (s0 + s1) / 2).abs() < 0.05);
        assert!((value(&e, t, s0) + 12.0).abs() < 1e-3);
        e.execute(
            "automation.draw",
            &json!({"tracks": [t.0], "param": "volume", "shape": "saw", "start": s0, "end": s1, "baseline": -6.0, "amplitude": 6.0, "cycles": 1}),
        )
        .unwrap();
        assert!((value(&e, t, s0) + 12.0).abs() < 1e-3);
        assert!((value(&e, t, (s0 + s1) / 2) + 6.0).abs() < 0.05);
        // Clamping: a line far above the range clamps at +12 dB.
        e.execute("automation.draw", &json!({"tracks": [t.0], "param": "volume", "shape": "line", "start": s0, "end": s1, "from": 40.0, "to": 40.0}))
            .unwrap();
        assert_eq!(value(&e, t, (s0 + s1) / 2), 12.0);
        // Pan lane untouched by volume drawing; non-finite rejected before any change.
        assert!(e.session().track(t).unwrap().lane(&AutoParam::Pan(0)).is_none());
        let before = e.session_arc();
        assert!(
            e.execute("automation.draw", &json!({"tracks": [t.0], "param": "volume", "shape": "line", "start": s0, "end": s1, "from": "nan"}))
                .is_err()
        );
        assert!(std::sync::Arc::ptr_eq(&before, &e.session_arc()));
        assert!(e.execute("automation.draw", &json!({"tracks": [t.0], "param": "volume", "shape": "blob", "start": s0, "end": s1})).is_err());
    }

    /// Freehand writes the given points; Transform shifts/scales only the box contents and
    /// drops points that leave it.
    #[test]
    fn freehand_and_transform() {
        let (mut e, t) = engine();
        e.execute("automation.draw", &json!({"tracks": [t.0], "param": "volume", "shape": "freehand", "start": 0, "end": 10_000, "points": [[0, -10.0], [5000, -2.0], [10_000, -10.0], [20_000, 0.0]]})).unwrap();
        assert_eq!(value(&e, t, 5000), -2.0);
        assert_eq!(e.session().track(t).unwrap().lane(&AutoParam::Volume).unwrap().points.len(), 3, "the point outside the range is ignored");
        // Transform: shift by +1000 samples and scale values by 0.5 inside [0, 10000].
        e.execute("automation.draw", &json!({"tracks": [t.0], "param": "volume", "shape": "transform", "box_start": 0, "box_end": 10_000, "time_shift": 1000, "value_scale": 0.5})).unwrap();
        let pts: Vec<(i64, f32)> = e.session().track(t).unwrap().lane(&AutoParam::Volume).unwrap().points.iter().map(|q| (q.at, q.value)).collect();
        assert_eq!(pts, vec![(1000, -5.0), (6000, -1.0)], "the point shifted past the box end is dropped: {pts:?}");
        assert!(
            e.execute(
                "automation.draw",
                &json!({"tracks": [t.0], "param": "volume", "shape": "transform", "box_start": 0, "box_end": 10_000, "time_scale": 0.0})
            )
            .is_err()
        );
    }
}
