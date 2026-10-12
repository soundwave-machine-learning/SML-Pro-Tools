//! Deterministic TEST-ONLY providers for slot/graph/compensation fixtures (MIXER_SLOT_SPEC
//! SL-T). Compiled only for tests or with the `test-providers` feature; never listed in any
//! catalogue or browser and never a shipping sound model.
//!
//! Ids encode their fixed settings:
//! - `test:identity`
//! - `test:gain:<dB>` (also a settable `gain` parameter, dB)
//! - `test:delay:<samples>` (reports that latency)
//! - `test:affine:<a>:<b>` (y = a·x + b; noncommuting with gain, so it proves order)
//! - `test:sidegate:<dB>` (passes the input only where |sidechain| exceeds the threshold; silent
//!   without a sidechain — proves sidechain routing and its alignment)
//! - `test:tail:<samples>` (input plus one echo that many samples later, no latency; declares
//!   that tail — proves offline tail handling)

use soundcraft_dsp::{Category, Plugin, PluginInfo};

pub const PREFIX: &str = "test:";

static INFO: PluginInfo = PluginInfo {
    id: "test:fixture",
    name: "Test fixture (not a sound model)",
    short_name: "TEST",
    category: Category::Other,
    params: &[],
    is_instrument: false,
    audiosuite: false,
};

enum Kind {
    Identity,
    Gain(f32),
    Delay(usize),
    Affine(f32, f32),
    SideGate(f32),
    Tail(usize),
}

struct Fixture {
    kind: Kind,
    rings: Vec<Vec<f32>>,
    idx: usize,
}

impl Plugin for Fixture {
    fn info(&self) -> &'static PluginInfo {
        &INFO
    }
    fn prepare(&mut self, _sr: f32, _mb: usize, ch: usize) {
        if let Kind::Delay(n) | Kind::Tail(n) = self.kind {
            self.rings = vec![vec![0.0; n.max(1)]; ch.max(1)];
            self.idx = 0;
        }
    }
    fn reset(&mut self) {
        for r in &mut self.rings {
            r.iter_mut().for_each(|x| *x = 0.0);
        }
        self.idx = 0;
    }
    /// `test:gain:*` exposes its gain as a settable `gain` parameter in dB.
    fn set_param(&mut self, id: &str, v: f32) -> bool {
        match (&mut self.kind, id) {
            (Kind::Gain(g), "gain") if v.is_finite() => {
                *g = 10f32.powf(v.clamp(-144.0, 24.0) / 20.0);
                true
            }
            _ => false,
        }
    }
    fn param(&self, id: &str) -> Option<f32> {
        match (&self.kind, id) {
            (Kind::Gain(g), "gain") => Some(20.0 * g.max(1e-9).log10()),
            _ => None,
        }
    }
    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        match self.kind {
            Kind::Identity => {}
            // Without a sidechain the gate stays closed.
            Kind::SideGate(_) => io.iter_mut().for_each(|c| c.iter_mut().take(frames).for_each(|x| *x = 0.0)),
            Kind::Tail(n) => {
                if n == 0 {
                    return;
                }
                let start = self.idx;
                for (c, ch) in io.iter_mut().enumerate() {
                    let Some(ring) = self.rings.get_mut(c) else { continue };
                    let mut i = start;
                    for x in ch.iter_mut().take(frames) {
                        if let Some(r) = ring.get_mut(i) {
                            let echo = *r;
                            *r = *x;
                            *x += echo;
                        }
                        i = (i + 1) % n;
                    }
                }
                self.idx = (start + frames) % n;
            }
            Kind::Gain(g) => io.iter_mut().for_each(|c| c.iter_mut().take(frames).for_each(|x| *x *= g)),
            Kind::Affine(a, b) => io.iter_mut().for_each(|c| c.iter_mut().take(frames).for_each(|x| *x = a * *x + b)),
            Kind::Delay(n) => {
                if n == 0 {
                    return;
                }
                let start = self.idx;
                for (c, ch) in io.iter_mut().enumerate() {
                    let Some(ring) = self.rings.get_mut(c) else { continue };
                    let mut i = start;
                    for x in ch.iter_mut().take(frames) {
                        if let Some(r) = ring.get_mut(i) {
                            std::mem::swap(r, x);
                        }
                        i = (i + 1) % n;
                    }
                }
                self.idx = (start + frames) % n;
            }
        }
    }
    fn latency(&self) -> usize {
        if let Kind::Delay(n) = self.kind { n } else { 0 }
    }
    fn tail_samples(&self) -> usize {
        if let Kind::Tail(n) = self.kind { n } else { 0 }
    }
    fn accepts_sidechain(&self) -> bool {
        matches!(self.kind, Kind::SideGate(_))
    }
    fn process_sidechain(&mut self, io: &mut [Vec<f32>], side: &[Vec<f32>], frames: usize) {
        let Kind::SideGate(thr) = self.kind else { return self.process(io, frames) };
        for (c, ch) in io.iter_mut().enumerate() {
            let key = side.get(c.min(side.len().saturating_sub(1)));
            for (i, x) in ch.iter_mut().take(frames).enumerate() {
                let open = key.and_then(|k| k.get(i)).is_some_and(|k| k.abs() > thr);
                if !open {
                    *x = 0.0;
                }
            }
        }
    }
}

/// Creates a test fixture from its id, or `None` for any other id.
pub fn create(id: &str) -> Option<Box<dyn Plugin>> {
    let rest = id.strip_prefix(PREFIX)?;
    let mut it = rest.split(':');
    let num = |s: Option<&str>| s.and_then(|x| x.parse::<f32>().ok()).filter(|v| v.is_finite());
    let kind = match it.next()? {
        "identity" => Kind::Identity,
        "gain" => Kind::Gain(10f32.powf(num(it.next())? / 20.0)),
        "delay" => Kind::Delay(it.next()?.parse::<usize>().ok().filter(|n| *n <= 1 << 16)?),
        "affine" => Kind::Affine(num(it.next())?, num(it.next())?),
        "sidegate" => Kind::SideGate(10f32.powf(num(it.next())? / 20.0)),
        "tail" => Kind::Tail(it.next()?.parse::<usize>().ok().filter(|n| *n <= 1 << 16)?),
        _ => return None,
    };
    Some(Box::new(Fixture { kind, rings: Vec::new(), idx: 0 }))
}
