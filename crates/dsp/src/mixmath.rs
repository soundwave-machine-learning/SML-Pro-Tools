//! SML mixer math (MIXER_MODES_SPEC M02, VISUAL_MIXER_SPEC VM03): the Logic-style profile pan
//! laws, stereo balance, dual stereo pan and native Stereo Width.
//!
//! These are original SML interpolation choices with published coefficients — NOT a claim of
//! equivalence with any other product's curves. `p` is in [-1, 1], `u = (p + 1) / 2`:
//! - `-3 dB`: (cos(πu/2), sin(πu/2)); centre ≈ -3.0103 dB, endpoints exact 0 / 1.
//! - `-3 dB compensated`: the -3 pair × √2 (centre 0 dB, endpoints +3.01 dB on the near side).
//! - `0 dB`: unity on the nearer side, cos(π|p|/2) on the opposite side.
//!
//! Width: M = (L+R)/2, S = (L−R)/2, L' = M + wS, R' = M − wS, w ∈ [0, 2]. At w = 1 the samples
//! pass through untouched (no M/S arithmetic), so 100 % is an exact identity.

use std::f64::consts::{FRAC_PI_2, SQRT_2};

/// Pan law of the SML mix profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ProfileLaw {
    /// 0 dB centre: near side unity, far side cos(π|p|/2).
    Zero,
    /// Equal power, -3.0103 dB centre (Logic-style preset default).
    #[default]
    Minus3,
    /// Equal power × √2: 0 dB centre.
    Minus3Compensated,
}

impl ProfileLaw {
    pub fn id(self) -> &'static str {
        match self {
            ProfileLaw::Zero => "0db",
            ProfileLaw::Minus3 => "-3db",
            ProfileLaw::Minus3Compensated => "-3db_compensated",
        }
    }
    pub fn parse(s: &str) -> Option<ProfileLaw> {
        Some(match s {
            "0db" | "0" | "zero" => ProfileLaw::Zero,
            "-3db" | "-3" | "minus3" => ProfileLaw::Minus3,
            "-3db_compensated" | "-3c" | "minus3_compensated" => ProfileLaw::Minus3Compensated,
            _ => return None,
        })
    }
}

fn clamp_p(p: f64) -> f64 {
    if p.is_nan() { 0.0 } else { p.clamp(-1.0, 1.0) }
}

/// Mono panner (L, R) gains.
pub fn mono_pan(p: f64, law: ProfileLaw) -> (f64, f64) {
    let p = clamp_p(p);
    match law {
        ProfileLaw::Zero => {
            let far = (FRAC_PI_2 * p.abs()).cos().max(0.0);
            // Exact zero at the hard endpoint (cos(π/2) is ~6e-17 in floating point).
            let far = if p.abs() >= 1.0 { 0.0 } else { far };
            if p < 0.0 {
                (1.0, far)
            } else if p > 0.0 {
                (far, 1.0)
            } else {
                (1.0, 1.0)
            }
        }
        ProfileLaw::Minus3 | ProfileLaw::Minus3Compensated => {
            let u = (p + 1.0) / 2.0;
            let (mut l, mut r) = ((FRAC_PI_2 * u).cos(), (FRAC_PI_2 * u).sin());
            if p <= -1.0 {
                (l, r) = (1.0, 0.0);
            } else if p >= 1.0 {
                (l, r) = (0.0, 1.0);
            }
            if law == ProfileLaw::Minus3Compensated { (l * SQRT_2, r * SQRT_2) } else { (l, r) }
        }
    }
}

/// Stereo balance: attenuates the opposite side, no crossfeed. Uncompensated uses the 0-law
/// gains; with `compensated` the selected mono-law pair is used independently on L and R.
pub fn balance(p: f64, law: ProfileLaw, compensated: bool) -> (f64, f64) {
    if compensated { mono_pan(p, law) } else { mono_pan(p, ProfileLaw::Zero) }
}

/// Dual stereo pan: each input side through its own mono panner. Returns the 2×2 matrix
/// `[[L→L, L→R], [R→L, R→R]]`.
pub fn stereo_pan(p_left: f64, p_right: f64, law: ProfileLaw) -> [[f64; 2]; 2] {
    let (ll, lr) = mono_pan(p_left, law);
    let (rl, rr) = mono_pan(p_right, law);
    [[ll, lr], [rl, rr]]
}

/// Visual-mixer horizontal drag in stereo-pan mode (VM02): centre/span with the span narrowed
/// at the edges. `start` is the gesture's starting endpoints; returns new endpoints and whether
/// the span was narrowed.
pub fn drag_stereo_center(start: (f64, f64), new_center: f64) -> ((f64, f64), bool) {
    let (pl, pr) = (clamp_p(start.0), clamp_p(start.1));
    let h = (pr - pl).abs() / 2.0;
    let sign = if pr >= pl { 1.0 } else { -1.0 };
    let c = clamp_p(new_center);
    let eff = h.min(1.0 - c.abs());
    ((c - sign * eff, c + sign * eff), eff < h)
}

/// Applies native Stereo Width `w` (0..=2) in place to a stereo pair. w == 1 is an exact
/// pass-through (the function returns without touching samples).
pub fn apply_width(l: &mut [f32], r: &mut [f32], w: f32) {
    if w == 1.0 || !w.is_finite() {
        return;
    }
    let w = w.clamp(0.0, 2.0);
    for (a, b) in l.iter_mut().zip(r.iter_mut()) {
        let m = (*a + *b) * 0.5;
        let s = (*a - *b) * 0.5;
        *a = m + w * s;
        *b = m - w * s;
    }
}

/// Width with a per-sample linear ramp from `w0` to `w1` (10 ms smoothing is done by the
/// caller choosing the ramp length). Exact identity when both ends are 1.
pub fn apply_width_ramp(l: &mut [f32], r: &mut [f32], w0: f32, w1: f32) {
    if w0 == 1.0 && w1 == 1.0 {
        return;
    }
    let n = l.len().min(r.len()).max(1) as f32;
    for (i, (a, b)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
        let w = (w0 + (w1 - w0) * (i as f32 + 1.0) / n).clamp(0.0, 2.0);
        let m = (*a + *b) * 0.5;
        let s = (*a - *b) * 0.5;
        *a = m + w * s;
        *b = m - w * s;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db(x: f64) -> f64 {
        20.0 * x.log10()
    }

    /// M-T2: endpoints/centre within 0.01 dB of the specified coefficients; exact zeros compared
    /// absolutely (no dB at -inf).
    #[test]
    fn pan_law_coefficients() {
        let (l, r) = mono_pan(0.0, ProfileLaw::Minus3);
        assert!((db(l) + 3.0103).abs() < 0.01 && (db(r) + 3.0103).abs() < 0.01);
        assert_eq!(mono_pan(-1.0, ProfileLaw::Minus3), (1.0, 0.0));
        assert_eq!(mono_pan(1.0, ProfileLaw::Minus3), (0.0, 1.0));
        let (l, r) = mono_pan(0.0, ProfileLaw::Minus3Compensated);
        assert!(db(l).abs() < 0.01 && db(r).abs() < 0.01);
        assert_eq!(mono_pan(0.0, ProfileLaw::Zero), (1.0, 1.0));
        assert_eq!(mono_pan(1.0, ProfileLaw::Zero), (0.0, 1.0));
        assert_eq!(mono_pan(-1.0, ProfileLaw::Zero), (1.0, 0.0));
        // Intermediate: p = 0.5 -> u = 0.75.
        let (l, r) = mono_pan(0.5, ProfileLaw::Minus3);
        assert!((l - (FRAC_PI_2 * 0.75).cos()).abs() < 1e-15 && (r - (FRAC_PI_2 * 0.75).sin()).abs() < 1e-15);
        let (l, r) = mono_pan(0.5, ProfileLaw::Zero);
        assert!((l - (FRAC_PI_2 * 0.5).cos()).abs() < 1e-15 && r == 1.0);
        // Equal power holds across a sweep.
        for k in -100..=100 {
            let (l, r) = mono_pan(f64::from(k) / 100.0, ProfileLaw::Minus3);
            assert!((l * l + r * r - 1.0).abs() < 1e-12);
        }
        assert_eq!(mono_pan(f64::NAN, ProfileLaw::Zero), (1.0, 1.0));
    }

    #[test]
    fn balance_has_no_crossfeed_and_stereo_pan_routes_each_side() {
        assert_eq!(balance(0.0, ProfileLaw::Minus3, false), (1.0, 1.0), "uncompensated balance is unity at centre");
        let (l, r) = balance(0.5, ProfileLaw::Minus3, false);
        assert!(l < 1.0 && r == 1.0);
        let m = stereo_pan(-1.0, 1.0, ProfileLaw::Minus3);
        assert_eq!(m, [[1.0, 0.0], [0.0, 1.0]], "full-width stereo pan is identity routing");
        let m = stereo_pan(1.0, 1.0, ProfileLaw::Minus3);
        assert_eq!(m, [[0.0, 1.0], [0.0, 1.0]]);
    }

    /// VM02: a full-width node can still move; the span narrows at the edge and reports it.
    #[test]
    fn full_width_stereo_node_can_move() {
        let ((l, r), narrowed) = drag_stereo_center((-1.0, 1.0), 0.5);
        assert!((l - 0.0).abs() < 1e-12 && (r - 1.0).abs() < 1e-12 && narrowed);
        assert!((((l + r) / 2.0) - 0.5).abs() < 1e-12, "centre moved");
        let ((l, r), narrowed) = drag_stereo_center((-0.2, 0.2), 0.3);
        assert!((l - 0.1).abs() < 1e-12 && (r - 0.5).abs() < 1e-12 && !narrowed);
        // Crossing back restores the starting span within the gesture.
        let (back, _) = drag_stereo_center((-1.0, 1.0), 0.0);
        assert_eq!(back, (-1.0, 1.0));
        // Reversed channel order keeps its sign.
        let ((l, r), _) = drag_stereo_center((0.4, -0.4), 0.0);
        assert!(l > r);
    }

    /// VM-T3: width formula, exact identity at 100 %, mono compatibility, side removal at 0.
    #[test]
    fn width_formula_and_exact_identity() {
        let l0: Vec<f32> = vec![1.0, 0.5, 1e-8, -0.25, 0.0];
        let r0: Vec<f32> = vec![0.0, 0.5, 1.0, 0.25, -1.0];
        let (mut l, mut r) = (l0.clone(), r0.clone());
        apply_width(&mut l, &mut r, 1.0);
        assert_eq!((l.clone(), r.clone()), (l0.clone(), r0.clone()), "100% is bit-exact, keeps the 1e-8 sample");
        for w in [0.0f32, 2.0, 0.5] {
            let (mut l, mut r) = (l0.clone(), r0.clone());
            apply_width(&mut l, &mut r, w);
            for i in 0..l0.len() {
                let m = (l0[i] + r0[i]) * 0.5;
                let s = (l0[i] - r0[i]) * 0.5;
                assert!((l[i] - (m + w * s)).abs() <= 1e-6 && (r[i] - (m - w * s)).abs() <= 1e-6);
                assert!(((l[i] + r[i]) * 0.5 - m).abs() <= 1e-6, "mono sum unchanged");
                if w == 0.0 {
                    assert_eq!(l[i], r[i], "w=0 removes side");
                }
            }
        }
        // 200 % on opposite-polarity input exceeds 0 dBFS without clipping.
        let (mut l, mut r) = (vec![1.0f32], vec![-1.0f32]);
        apply_width(&mut l, &mut r, 2.0);
        assert_eq!((l[0], r[0]), (2.0, -2.0));
        let (mut l, mut r) = (l0.clone(), r0.clone());
        apply_width_ramp(&mut l, &mut r, 1.0, 1.0);
        assert_eq!(l, l0);
    }
}
