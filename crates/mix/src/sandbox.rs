//! Sandboxed hosting (R31 "compatibility mode", decided in E03): one hosted plugin instance runs
//! in a worker process; the audio thread exchanges one block per callback with it over pipes,
//! **one block ahead**, so a slow reply costs a counted miss instead of a stall (a worker
//! that answers nothing for a second while blocks are outstanding is hung). Declared added
//! latency is exactly one block (`max_block`). A worker that crashes or stops answering is a
//! visible fault: the slot goes silent (bounded: the current block), the fault is logged, and
//! the instance is restarted with its last captured state unless auto-restart is off (the
//! playback host turns it off while recording — a crashed processor is never silently resumed
//! into a take). Opt-in per insert (`sandbox:<plugin id>`), never a default.
//!
//! What it does not do: host the plugin's native editor (generic parameters only), hide the
//! added block of latency, or make any claim about the plugin's sound. The plugin's static
//! description (parameters) is still read in-process once (R07 scan cache); only processing,
//! state and lifetime are isolated.
//!
//! Wire protocol (little-endian, over the worker's stdin/stdout): `tag u8` then
//! - 1 prepare `{sr f32, max_block u32, ch u32}` → `1, ok u8, latency u32, tail u32, side u8`
//! - 2 process `{frames u32, ch u32, side_ch u32, ch×frames f32, side_ch×frames f32}` →
//!   `2, frames u32, ch u32, ch×frames f32, latency u32, gr f32`
//! - 3 set_param `{len u16, id bytes, value f32}` (no reply)
//! - 4 save_state → `4, len u32, bytes`; 5 load_state `{len u32, bytes}` → `5, ok u8`
//! - 6 reset; 7 note_on `{offset u32, note u8, vel u8}`; 8 note_off `{offset u32, note u8}`;
//!   9 all_notes_off (no replies)

use soundcraft_dsp::{Plugin, PluginInfo};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError, sync_channel};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::{Duration, Instant, SystemTime};

pub const ID_PREFIX: &str = "sandbox:";
/// Longest block a worker accepts.
const MAX_FRAMES: usize = 1 << 14;
const MAX_CHANNELS: usize = 64;
/// Longest state blob accepted from a worker (hostile output bound).
const MAX_STATE: usize = 64 << 20;
/// A worker that has answered nothing for this long while blocks are outstanding is hung.
/// (Late-but-arriving replies under CPU starvation are counted, not mistaken for a hang.)
const HANG_TIMEOUT: Duration = Duration::from_secs(1);
/// Restarts per instance before it stays faulted.
const MAX_RESTARTS: u32 = 5;
/// How long the control thread waits for a state reply.
const STATE_TIMEOUT: Duration = Duration::from_secs(3);

/// The hosted id inside a `sandbox:` id.
pub fn inner_id(id: &str) -> Option<&str> {
    id.strip_prefix(ID_PREFIX).filter(|s| !s.is_empty())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Fault {
    None,
    Crashed,
    Hung,
}

/// One logged fault (what the user sees in Window › Plugin Faults / `engine.host_faults`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FaultEvent {
    pub plugin: String,
    pub fault: Fault,
    pub at: SystemTime,
    /// The instance was restarted with its last captured state.
    pub restarted: bool,
    pub restarts: u32,
}

/// Live status of one sandboxed instance.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct InstanceStatus {
    pub plugin: String,
    pub pid: u32,
    pub fault: Fault,
    pub blocks: u64,
    /// Blocks whose reply was not there in time (silence substituted for that block).
    pub late_blocks: u64,
    pub restarts: u32,
    /// Worker resident set size in KiB (Linux `/proc`; 0 elsewhere).
    pub rss_kib: u64,
    pub added_latency_samples: usize,
}

static FAULTS: Mutex<Vec<FaultEvent>> = Mutex::new(Vec::new());
static AUTO_RESTART: AtomicBool = AtomicBool::new(true);
static INSTANCES: Mutex<Vec<Weak<Shared>>> = Mutex::new(Vec::new());
static WORKER: Mutex<Option<(PathBuf, Vec<String>)>> = Mutex::new(None);
static FAULT_INJECT: Mutex<Option<String>> = Mutex::new(None);

/// Fault tests only: workers spawned from now on get `--fault=crash:<n>` / `--fault=hang:<n>`
/// (abort / stop answering after `n` blocks). `None` clears it.
pub fn set_fault_injection(spec: Option<&str>) {
    *FAULT_INJECT.lock().unwrap_or_else(PoisonError::into_inner) = spec.map(str::to_string);
}

/// Faults logged since start (bounded).
pub fn faults() -> Vec<FaultEvent> {
    FAULTS.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

/// Whether a faulted worker is restarted automatically. The playback host turns this off while
/// recording: a crashed processor is never silently resumed into a take.
pub fn set_auto_restart(on: bool) {
    AUTO_RESTART.store(on, Ordering::Release);
}

pub fn auto_restart() -> bool {
    AUTO_RESTART.load(Ordering::Acquire)
}

/// The worker program and the arguments placed before the plugin id (default: this executable
/// with `--plugin-worker`). Tests point it at themselves.
pub fn set_worker(program: PathBuf, args: Vec<String>) {
    *WORKER.lock().unwrap_or_else(PoisonError::into_inner) = Some((program, args));
}

fn worker() -> Option<(PathBuf, Vec<String>)> {
    if let Some(w) = WORKER.lock().unwrap_or_else(PoisonError::into_inner).clone() {
        return Some(w);
    }
    Some((std::env::current_exe().ok()?, vec!["--plugin-worker".into()]))
}

/// Status of every live sandboxed instance.
pub fn status() -> Vec<InstanceStatus> {
    let mut list = INSTANCES.lock().unwrap_or_else(PoisonError::into_inner);
    list.retain(|w| w.strong_count() > 0);
    list.iter()
        .filter_map(Weak::upgrade)
        .filter(|s| s.alive.load(Ordering::Acquire))
        .map(|s| {
            let pid = s.pid.load(Ordering::Relaxed);
            InstanceStatus {
                plugin: s.id.clone(),
                pid,
                fault: s.fault(),
                blocks: s.blocks.load(Ordering::Relaxed),
                late_blocks: s.misses.load(Ordering::Relaxed),
                restarts: s.restarts.load(Ordering::Relaxed),
                rss_kib: rss_kib(pid),
                added_latency_samples: s.config.lock().unwrap_or_else(PoisonError::into_inner).map_or(0, |c| c.1),
            }
        })
        .collect()
}

fn rss_kib(pid: u32) -> u64 {
    if pid == 0 {
        return 0;
    }
    let Ok(s) = std::fs::read_to_string(format!("/proc/{pid}/statm")) else { return 0 };
    let pages: u64 = s.split_whitespace().nth(1).and_then(|x| x.parse().ok()).unwrap_or(0);
    pages * 4
}

fn log_fault(ev: FaultEvent) {
    log::warn!("sandbox: {} {:?} (restarted: {})", ev.plugin, ev.fault, ev.restarted);
    let mut f = FAULTS.lock().unwrap_or_else(PoisonError::into_inner);
    if f.len() >= 256 {
        f.remove(0);
    }
    f.push(ev);
}

/// State shared between the audio-thread plugin, its restarter and the status query.
struct Shared {
    id: String,
    inner: String,
    fault: AtomicU8,
    misses: AtomicU64,
    blocks: AtomicU64,
    restarts: AtomicU32,
    pid: AtomicU32,
    alive: AtomicBool,
    /// (sample rate, max block, channels) of the last prepare.
    config: Mutex<Option<(f32, usize, usize)>>,
    last_state: Mutex<Option<Vec<u8>>>,
    params: Mutex<Vec<(String, f32)>>,
}

impl Shared {
    fn fault(&self) -> Fault {
        match self.fault.load(Ordering::Acquire) {
            1 => Fault::Crashed,
            2 => Fault::Hung,
            _ => Fault::None,
        }
    }
    fn set_fault(&self, f: Fault) {
        self.fault.store(
            match f {
                Fault::None => 0,
                Fault::Crashed => 1,
                Fault::Hung => 2,
            },
            Ordering::Release,
        );
    }
}

struct Reply {
    tag: u8,
    frames: usize,
    ch: usize,
    samples: Vec<f32>,
    latency: u32,
    gr: f32,
    blob: Vec<u8>,
    ok: bool,
}

struct Link {
    child: Child,
    stdin: ChildStdin,
    audio_rx: Receiver<Reply>,
    ctl_rx: Receiver<Reply>,
    recycle: SyncSender<Vec<f32>>,
}

impl Drop for Link {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn_link(inner: &str) -> Result<Link, String> {
    let (program, args) = worker().ok_or("no worker program")?;
    let mut cmd = Command::new(program);
    cmd.args(args).arg(inner);
    for d in soundcraft_clap_host::extra_search_dirs().into_iter().chain(soundcraft_vst3_host::extra_search_dirs()) {
        cmd.arg(d);
    }
    if let Some(f) = FAULT_INJECT.lock().unwrap_or_else(PoisonError::into_inner).clone() {
        cmd.arg(format!("--fault={f}"));
    }
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).spawn().map_err(|e| e.to_string())?;
    let stdin = child.stdin.take().ok_or("no stdin")?;
    let mut stdout = child.stdout.take().ok_or("no stdout")?;
    let (audio_tx, audio_rx) = sync_channel::<Reply>(4);
    let (ctl_tx, ctl_rx) = sync_channel::<Reply>(4);
    let (recycle_tx, recycle_rx) = sync_channel::<Vec<f32>>(8);
    // Replies are read by this thread; the audio thread only takes from the channel.
    let _ = std::thread::Builder::new().name("sml-sandbox-reader".into()).spawn(move || {
        let mut pool: Vec<Vec<f32>> = (0..4).map(|_| Vec::with_capacity(MAX_CHANNELS.min(8) * 1024)).collect();
        loop {
            let mut tag = [0u8; 1];
            if stdout.read_exact(&mut tag).is_err() {
                return;
            }
            while let Ok(v) = recycle_rx.try_recv() {
                pool.push(v);
            }
            let reply = match tag[0] {
                1 => {
                    let mut b = [0u8; 10];
                    if stdout.read_exact(&mut b).is_err() {
                        return;
                    }
                    Reply {
                        tag: 1,
                        ok: b[0] == 1,
                        latency: u32::from_le_bytes([b[1], b[2], b[3], b[4]]),
                        frames: u32::from_le_bytes([b[5], b[6], b[7], b[8]]) as usize,
                        ch: usize::from(b[9]),
                        samples: Vec::new(),
                        gr: 0.0,
                        blob: Vec::new(),
                    }
                }
                2 => {
                    let mut h = [0u8; 8];
                    if stdout.read_exact(&mut h).is_err() {
                        return;
                    }
                    let frames = u32::from_le_bytes([h[0], h[1], h[2], h[3]]) as usize;
                    let ch = u32::from_le_bytes([h[4], h[5], h[6], h[7]]) as usize;
                    if frames > MAX_FRAMES || ch > MAX_CHANNELS {
                        return;
                    }
                    let mut samples = pool.pop().unwrap_or_default();
                    samples.clear();
                    samples.resize(frames * ch, 0.0);
                    let mut bytes = vec![0u8; frames * ch * 4];
                    if stdout.read_exact(&mut bytes).is_err() {
                        return;
                    }
                    for (s, b) in samples.iter_mut().zip(bytes.chunks_exact(4)) {
                        *s = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                    }
                    let mut t = [0u8; 8];
                    if stdout.read_exact(&mut t).is_err() {
                        return;
                    }
                    Reply {
                        tag: 2,
                        frames,
                        ch,
                        samples,
                        latency: u32::from_le_bytes([t[0], t[1], t[2], t[3]]),
                        gr: f32::from_le_bytes([t[4], t[5], t[6], t[7]]),
                        blob: Vec::new(),
                        ok: true,
                    }
                }
                4 => {
                    let mut l = [0u8; 4];
                    if stdout.read_exact(&mut l).is_err() {
                        return;
                    }
                    let len = u32::from_le_bytes(l) as usize;
                    if len > MAX_STATE {
                        return;
                    }
                    let mut blob = vec![0u8; len];
                    if stdout.read_exact(&mut blob).is_err() {
                        return;
                    }
                    Reply { tag: 4, frames: 0, ch: 0, samples: Vec::new(), latency: 0, gr: 0.0, blob, ok: true }
                }
                5 => {
                    let mut b = [0u8; 1];
                    if stdout.read_exact(&mut b).is_err() {
                        return;
                    }
                    Reply { tag: 5, frames: 0, ch: 0, samples: Vec::new(), latency: 0, gr: 0.0, blob: Vec::new(), ok: b[0] == 1 }
                }
                _ => return,
            };
            let r = if reply.tag == 2 { audio_tx.send(reply) } else { ctl_tx.send(reply) };
            if r.is_err() {
                return;
            }
        }
    });
    Ok(Link { child, stdin, audio_rx, ctl_rx, recycle: recycle_tx })
}

fn wait_ctl(link: &Link, tag: u8) -> Option<Reply> {
    let deadline = Instant::now() + STATE_TIMEOUT;
    loop {
        let now = Instant::now();
        if now >= deadline {
            return None;
        }
        match link.ctl_rx.recv_timeout(deadline - now) {
            Ok(r) if r.tag == tag => return Some(r),
            Ok(_) => continue,
            Err(_) => return None,
        }
    }
}

/// Sends prepare and waits for the worker's answer: (latency, tail, accepts sidechain).
fn prepare_link(link: &mut Link, sr: f32, mb: usize, ch: usize) -> Result<(usize, usize, bool), String> {
    let mut req = Vec::with_capacity(16);
    req.push(1u8);
    req.extend_from_slice(&sr.to_le_bytes());
    req.extend_from_slice(&(mb as u32).to_le_bytes());
    req.extend_from_slice(&(ch as u32).to_le_bytes());
    link.stdin.write_all(&req).and_then(|()| link.stdin.flush()).map_err(|e| e.to_string())?;
    let r = wait_ctl(link, 1).ok_or("the worker did not answer prepare")?;
    if !r.ok {
        return Err("the worker could not create the plugin".into());
    }
    Ok((r.latency as usize, r.frames, r.ch == 1))
}

/// A silent block sent ahead so the first callback already finds a reply (one-block lead).
fn prime(link: &mut Link, mb: usize, ch: usize) -> Result<(), String> {
    let mut req = Vec::with_capacity(16 + mb * ch * 4);
    req.push(2u8);
    req.extend_from_slice(&(mb as u32).to_le_bytes());
    req.extend_from_slice(&(ch as u32).to_le_bytes());
    req.extend_from_slice(&0u32.to_le_bytes());
    req.resize(req.len() + mb * ch * 4, 0);
    link.stdin.write_all(&req).and_then(|()| link.stdin.flush()).map_err(|e| e.to_string())
}

/// The audio-thread side of one sandboxed instance.
pub struct SandboxPlugin {
    info: &'static PluginInfo,
    shared: Arc<Shared>,
    link: Arc<Mutex<Option<Link>>>,
    req: Vec<u8>,
    fifo: Vec<VecDeque<f32>>,
    /// Samples substituted with silence whose late replies must be discarded (keeps timing).
    owed: usize,
    sr: f32,
    mb: usize,
    ch: usize,
    values: Vec<f32>,
    plugin_latency: usize,
    tail: usize,
    accepts_side: bool,
    gr: f32,
    consecutive_late: u32,
    /// When the worker last answered a block (hang detection is wall-clock based).
    last_reply: Instant,
    /// Blocks sent and not yet answered. Bounded so a stalled worker never fills the pipe and
    /// blocks the audio thread on a write.
    in_flight: u32,
    /// Worker pid this side last synchronised with (a change means a restart happened).
    seen_pid: u32,
    /// Offline render: wait for every reply (bounded by `STATE_TIMEOUT`) instead of
    /// substituting silence after half a block — an export is never a substituted result.
    offline: bool,
    /// Blocks replaced by silence since creation ([`Plugin::substituted_blocks`]).
    substituted: u64,
}

/// Most unanswered blocks before the host stops sending (silence instead).
const MAX_IN_FLIGHT: u32 = 3;

/// Creates a sandboxed instance of `sandbox:<id>` (spawns the worker; control thread only).
pub fn create(id: &str) -> Option<Box<dyn Plugin>> {
    let inner = inner_id(id)?;
    let info = soundcraft_clap_host::plugin_info(inner)
        .or_else(|| soundcraft_vst3_host::plugin_info(inner))
        .or_else(|| soundcraft_au_host::plugin_info(inner))?;
    let link = match spawn_link(inner) {
        Ok(l) => l,
        Err(e) => {
            log::warn!("sandbox: cannot start a worker for {inner}: {e}");
            return None;
        }
    };
    let shared = Arc::new(Shared {
        id: id.to_string(),
        inner: inner.to_string(),
        fault: AtomicU8::new(0),
        misses: AtomicU64::new(0),
        blocks: AtomicU64::new(0),
        restarts: AtomicU32::new(0),
        pid: AtomicU32::new(link.child.id()),
        alive: AtomicBool::new(true),
        config: Mutex::new(None),
        last_state: Mutex::new(None),
        params: Mutex::new(Vec::new()),
    });
    INSTANCES.lock().unwrap_or_else(PoisonError::into_inner).push(Arc::downgrade(&shared));
    let link = Arc::new(Mutex::new(Some(link)));
    spawn_restarter(Arc::clone(&shared), Arc::clone(&link));
    Some(Box::new(SandboxPlugin {
        info,
        shared,
        link,
        req: Vec::new(),
        fifo: Vec::new(),
        owed: 0,
        sr: 48_000.0,
        mb: 0,
        ch: 0,
        values: info.params.iter().map(|p| p.default).collect(),
        plugin_latency: 0,
        tail: 0,
        accepts_side: false,
        gr: 0.0,
        consecutive_late: 0,
        last_reply: Instant::now(),
        in_flight: 0,
        seen_pid: 0,
        offline: false,
        substituted: 0,
    }))
}

/// Watches for a fault and, when allowed, restarts the worker with the last captured state and
/// the applied parameters. Runs until the instance is dropped.
fn spawn_restarter(shared: Arc<Shared>, link: Arc<Mutex<Option<Link>>>) {
    let _ = std::thread::Builder::new().name("sml-sandbox-restart".into()).spawn(move || {
        while shared.alive.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(50));
            if shared.fault() == Fault::None {
                continue;
            }
            if !auto_restart() || shared.restarts.load(Ordering::Relaxed) >= MAX_RESTARTS {
                continue;
            }
            let Some(cfg) = *shared.config.lock().unwrap_or_else(PoisonError::into_inner) else { continue };
            let mut guard = link.lock().unwrap_or_else(PoisonError::into_inner);
            let mut fresh = match spawn_link(&shared.inner) {
                Ok(l) => l,
                Err(e) => {
                    log::warn!("sandbox: restart of {} failed: {e}", shared.inner);
                    shared.restarts.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
            };
            let ok = prepare_link(&mut fresh, cfg.0, cfg.1, cfg.2).is_ok();
            if ok {
                if let Some(state) = shared.last_state.lock().unwrap_or_else(PoisonError::into_inner).clone() {
                    let mut req = Vec::with_capacity(5 + state.len());
                    req.push(5u8);
                    req.extend_from_slice(&(state.len() as u32).to_le_bytes());
                    req.extend_from_slice(&state);
                    let _ = fresh.stdin.write_all(&req).and_then(|()| fresh.stdin.flush());
                    let _ = wait_ctl(&fresh, 5);
                }
                for (k, v) in shared.params.lock().unwrap_or_else(PoisonError::into_inner).iter() {
                    let _ = fresh.stdin.write_all(&param_request(k, *v));
                }
                let _ = fresh.stdin.flush();
                let _ = prime(&mut fresh, cfg.1, cfg.2);
            }
            let n = shared.restarts.fetch_add(1, Ordering::Relaxed) + 1;
            let fault = shared.fault();
            if ok {
                shared.pid.store(fresh.child.id(), Ordering::Relaxed);
                *guard = Some(fresh);
                shared.set_fault(Fault::None);
            }
            log_fault(FaultEvent { plugin: shared.id.clone(), fault, at: SystemTime::now(), restarted: ok, restarts: n });
        }
    });
}

fn param_request(id: &str, v: f32) -> Vec<u8> {
    let bytes = id.as_bytes();
    let mut req = Vec::with_capacity(7 + bytes.len());
    req.push(3u8);
    req.extend_from_slice(&(bytes.len().min(u16::MAX as usize) as u16).to_le_bytes());
    req.extend_from_slice(&bytes[..bytes.len().min(u16::MAX as usize)]);
    req.extend_from_slice(&v.to_le_bytes());
    req
}

impl SandboxPlugin {
    fn fault(&mut self, link: &mut Link, f: Fault) {
        if self.shared.fault() == Fault::None {
            self.shared.set_fault(f);
            let _ = link.child.kill();
            log_fault(FaultEvent {
                plugin: self.shared.id.clone(),
                fault: f,
                at: SystemTime::now(),
                restarted: false,
                restarts: self.shared.restarts.load(Ordering::Relaxed),
            });
        }
    }

    /// This block becomes silence in place of the plugin's output (counted: an offline render
    /// with substituted blocks is not the mix and refuses to export).
    fn silence(&mut self, io: &mut [Vec<f32>], frames: usize) {
        self.substituted = self.substituted.saturating_add(1);
        for c in io.iter_mut() {
            c.iter_mut().take(frames).for_each(|x| *x = 0.0);
        }
    }

    /// Exchanges one block: sends `io` (and `side`), takes the previous block's reply.
    fn exchange(&mut self, io: &mut [Vec<f32>], side: Option<&[Vec<f32>]>, frames: usize) {
        let frames = frames.min(self.mb).min(io.iter().map(Vec::len).min().unwrap_or(0));
        if frames == 0 || self.mb == 0 {
            return;
        }
        if self.shared.fault() != Fault::None {
            self.silence(io, frames);
            return;
        }
        let link_arc = Arc::clone(&self.link);
        let Ok(mut guard) = link_arc.try_lock() else {
            // A restart is swapping the worker: this block is silent.
            self.silence(io, frames);
            return;
        };
        let Some(link) = guard.as_mut() else {
            self.silence(io, frames);
            return;
        };
        if self.shared.pid.load(Ordering::Relaxed) != self.seen_pid {
            // A restarted worker: start the lead accounting afresh (its prime block is in flight).
            self.seen_pid = self.shared.pid.load(Ordering::Relaxed);
            self.in_flight = 1;
            self.owed = 0;
            self.consecutive_late = 0;
            self.last_reply = Instant::now();
            for q in &mut self.fifo {
                q.clear();
            }
        }
        let ch = self.ch.min(io.len());
        let side_ch = if self.accepts_side { side.map_or(0, |s| s.len().min(MAX_CHANNELS)) } else { 0 };
        // Drain replies that arrived meanwhile (a worker catching up after late blocks).
        while self.in_flight > 1 {
            match link.audio_rx.try_recv() {
                Ok(r) => {
                    self.in_flight -= 1;
                    self.last_reply = Instant::now();
                    self.consecutive_late = 0;
                    let skip = self.owed.min(r.frames);
                    self.owed -= skip;
                    let _ = link.recycle.try_send(r.samples);
                }
                Err(_) => break,
            }
        }
        if self.in_flight >= MAX_IN_FLIGHT && self.offline {
            // Offline: wait for the worker to catch up rather than skipping blocks.
            match link.audio_rx.recv_timeout(STATE_TIMEOUT) {
                Ok(r) => {
                    self.in_flight -= 1;
                    self.last_reply = Instant::now();
                    let skip = self.owed.min(r.frames);
                    self.owed -= skip;
                    let _ = link.recycle.try_send(r.samples);
                }
                Err(_) => {
                    self.fault(link, Fault::Hung);
                    self.silence(io, frames);
                    return;
                }
            }
        }
        if self.in_flight >= MAX_IN_FLIGHT {
            // The worker is behind: this block is silent and counted; nothing is written, so
            // the pipe can never fill and block this thread.
            self.shared.misses.fetch_add(1, Ordering::Relaxed);
            self.consecutive_late += 1;
            if link.child.try_wait().ok().flatten().is_some() {
                self.fault(link, Fault::Crashed);
            } else if self.last_reply.elapsed() >= HANG_TIMEOUT {
                self.fault(link, Fault::Hung);
            }
            self.owed = self.owed.saturating_add(frames);
            self.silence(io, frames);
            return;
        }
        self.req.clear();
        self.req.push(2u8);
        self.req.extend_from_slice(&(frames as u32).to_le_bytes());
        self.req.extend_from_slice(&(ch as u32).to_le_bytes());
        self.req.extend_from_slice(&(side_ch as u32).to_le_bytes());
        for c in io.iter().take(ch) {
            for x in c.iter().take(frames) {
                self.req.extend_from_slice(&x.to_le_bytes());
            }
        }
        if let Some(side) = side {
            for c in side.iter().take(side_ch) {
                for i in 0..frames {
                    self.req.extend_from_slice(&c.get(i).copied().unwrap_or(0.0).to_le_bytes());
                }
            }
        }
        if link.stdin.write_all(&self.req).and_then(|()| link.stdin.flush()).is_err() {
            self.fault(link, Fault::Crashed);
            self.silence(io, frames);
            return;
        }
        self.shared.blocks.fetch_add(1, Ordering::Relaxed);
        self.in_flight += 1;
        if self.in_flight < 2 {
            // The lead block is already in the FIFO (prepare answered it): nothing to receive.
            for (c, out) in io.iter_mut().enumerate().take(ch) {
                let mut q = self.fifo.get_mut(c);
                for x in out.iter_mut().take(frames) {
                    *x = q.as_mut().and_then(|q| q.pop_front()).unwrap_or(0.0);
                }
            }
            return;
        }
        // The previous block's reply: normally already there; wait at most half a block period.
        let wait = if self.offline {
            STATE_TIMEOUT
        } else {
            Duration::from_secs_f64(f64::from(frames as u32) / f64::from(self.sr.max(1.0)) * 0.5).min(Duration::from_millis(5))
        };
        let reply = match link.audio_rx.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Disconnected) => None,
            Err(TryRecvError::Empty) => match link.audio_rx.recv_timeout(wait) {
                Ok(r) => Some(r),
                Err(RecvTimeoutError::Timeout) => {
                    self.shared.misses.fetch_add(1, Ordering::Relaxed);
                    self.consecutive_late += 1;
                    if link.child.try_wait().ok().flatten().is_some() {
                        self.fault(link, Fault::Crashed);
                    } else if self.last_reply.elapsed() >= HANG_TIMEOUT {
                        self.fault(link, Fault::Hung);
                    }
                    self.owed = self.owed.saturating_add(frames);
                    self.silence(io, frames);
                    return;
                }
                Err(RecvTimeoutError::Disconnected) => None,
            },
        };
        let Some(reply) = reply else {
            self.fault(link, Fault::Crashed);
            self.silence(io, frames);
            return;
        };
        self.consecutive_late = 0;
        self.last_reply = Instant::now();
        self.in_flight = self.in_flight.saturating_sub(1);
        self.plugin_latency = reply.latency as usize;
        self.gr = reply.gr;
        // Late replies that were already replaced by silence are discarded to keep timing.
        let skip = self.owed.min(reply.frames);
        self.owed -= skip;
        for (c, q) in self.fifo.iter_mut().enumerate().take(ch) {
            let rc = c.min(reply.ch.saturating_sub(1));
            for i in skip..reply.frames {
                if let Some(x) = reply.samples.get(rc * reply.frames + i)
                    && q.len() < q.capacity()
                {
                    q.push_back(*x);
                }
            }
        }
        let _ = link.recycle.try_send(reply.samples);
        for (c, out) in io.iter_mut().enumerate().take(ch) {
            let mut q = self.fifo.get_mut(c);
            for x in out.iter_mut().take(frames) {
                *x = q.as_mut().and_then(|q| q.pop_front()).unwrap_or(0.0);
            }
        }
    }
}

impl Drop for SandboxPlugin {
    fn drop(&mut self) {
        self.shared.alive.store(false, Ordering::Release);
        if let Ok(mut g) = self.link.lock() {
            *g = None;
        }
    }
}

impl Plugin for SandboxPlugin {
    fn info(&self) -> &'static PluginInfo {
        self.info
    }

    fn prepare(&mut self, sample_rate: f32, max_block: usize, channels: usize) {
        self.sr = if sample_rate.is_finite() && sample_rate > 0.0 { sample_rate } else { 48_000.0 };
        self.mb = max_block.clamp(1, MAX_FRAMES);
        self.ch = channels.clamp(1, MAX_CHANNELS);
        *self.shared.config.lock().unwrap_or_else(PoisonError::into_inner) = Some((self.sr, self.mb, self.ch));
        self.req = Vec::with_capacity(32 + 2 * self.ch * self.mb * 4 + MAX_CHANNELS.min(8) * self.mb * 4);
        self.fifo = (0..self.ch).map(|_| VecDeque::with_capacity(4 * self.mb)).collect();
        self.owed = 0;
        self.consecutive_late = 0;
        self.last_reply = Instant::now();
        let mut guard = self.link.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(link) = guard.as_mut() else { return };
        match prepare_link(link, self.sr, self.mb, self.ch) {
            Ok((lat, tail, side)) => {
                self.plugin_latency = lat;
                self.tail = tail;
                self.accepts_side = side;
                // The plugin now exists in the worker: restore the stored state, then the
                // parameters set so far on top (same order as in-process hosting).
                if let Some(state) = self.shared.last_state.lock().unwrap_or_else(PoisonError::into_inner).clone() {
                    let mut req = Vec::with_capacity(5 + state.len());
                    req.push(5u8);
                    req.extend_from_slice(&(state.len() as u32).to_le_bytes());
                    req.extend_from_slice(&state);
                    if link.stdin.write_all(&req).and_then(|()| link.stdin.flush()).is_ok() {
                        let _ = wait_ctl(link, 5);
                    }
                }
                let set: Vec<(String, f32)> = self.shared.params.lock().unwrap_or_else(PoisonError::into_inner).clone();
                for (k, v) in &set {
                    let _ = link.stdin.write_all(&param_request(k, *v));
                }
                let _ = link.stdin.flush();
                // One silent block ahead, answered before the first callback (worker warm-up
                // happens here, on the control thread, not in the callback).
                self.in_flight = 0;
                if prime(link, self.mb, self.ch).is_err() {
                    self.shared.set_fault(Fault::Crashed);
                } else {
                    match link.audio_rx.recv_timeout(STATE_TIMEOUT) {
                        Ok(r) => {
                            for (c, q) in self.fifo.iter_mut().enumerate().take(self.ch) {
                                let rc = c.min(r.ch.saturating_sub(1));
                                for i in 0..r.frames {
                                    if let Some(x) = r.samples.get(rc * r.frames + i) {
                                        q.push_back(*x);
                                    }
                                }
                            }
                            let _ = link.recycle.try_send(r.samples);
                        }
                        Err(_) => self.in_flight = 1,
                    }
                }
                self.seen_pid = self.shared.pid.load(Ordering::Relaxed);
            }
            Err(e) => {
                log::warn!("sandbox: {}: {e}", self.shared.inner);
                self.shared.set_fault(Fault::Crashed);
                log_fault(FaultEvent { plugin: self.shared.id.clone(), fault: Fault::Crashed, at: SystemTime::now(), restarted: false, restarts: 0 });
            }
        }
    }

    fn reset(&mut self) {
        if let Ok(mut g) = self.link.try_lock()
            && let Some(link) = g.as_mut()
        {
            let _ = link.stdin.write_all(&[6u8]).and_then(|()| link.stdin.flush());
        }
    }

    fn set_param(&mut self, id: &str, value: f32) -> bool {
        let Some(i) = self.info.params.iter().position(|p| p.id == id) else { return false };
        let v = self.info.params[i].clamp(value);
        if let Some(slot) = self.values.get_mut(i) {
            *slot = v;
        }
        if let Ok(mut ps) = self.shared.params.try_lock() {
            if let Some(e) = ps.iter_mut().find(|(k, _)| k == id) {
                e.1 = v;
            } else if ps.len() < 4096 {
                ps.push((id.to_string(), v));
            }
        }
        if let Ok(mut g) = self.link.try_lock()
            && let Some(link) = g.as_mut()
        {
            let _ = link.stdin.write_all(&param_request(id, v)).and_then(|()| link.stdin.flush());
        }
        true
    }

    fn param(&self, id: &str) -> Option<f32> {
        let i = self.info.params.iter().position(|p| p.id == id)?;
        self.values.get(i).copied()
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        self.exchange(io, None, frames);
    }

    fn accepts_sidechain(&self) -> bool {
        self.accepts_side
    }

    fn substituted_blocks(&self) -> u64 {
        self.substituted
    }

    fn set_offline(&mut self, offline: bool) {
        self.offline = offline;
    }

    fn process_sidechain(&mut self, io: &mut [Vec<f32>], side: &[Vec<f32>], frames: usize) {
        self.exchange(io, Some(side), frames);
    }

    /// One block of lead plus whatever the plugin declares.
    fn latency(&self) -> usize {
        self.mb.saturating_add(self.plugin_latency)
    }

    fn tail_samples(&self) -> usize {
        self.tail
    }

    fn gain_reduction_db(&self) -> f32 {
        self.gr
    }

    fn note_on(&mut self, offset: usize, note: u8, velocity: u8) {
        if let Ok(mut g) = self.link.try_lock()
            && let Some(link) = g.as_mut()
        {
            let mut req = [7u8, 0, 0, 0, 0, note, velocity];
            req[1..5].copy_from_slice(&(offset as u32).to_le_bytes());
            let _ = link.stdin.write_all(&req);
        }
    }

    fn note_off(&mut self, offset: usize, note: u8) {
        if let Ok(mut g) = self.link.try_lock()
            && let Some(link) = g.as_mut()
        {
            let mut req = [8u8, 0, 0, 0, 0, note];
            req[1..5].copy_from_slice(&(offset as u32).to_le_bytes());
            let _ = link.stdin.write_all(&req);
        }
    }

    fn all_notes_off(&mut self) {
        if let Ok(mut g) = self.link.try_lock()
            && let Some(link) = g.as_mut()
        {
            let _ = link.stdin.write_all(&[9u8]).and_then(|()| link.stdin.flush());
        }
    }

    /// The worker's state; while faulted, the last captured checkpoint (never nothing because of
    /// a crash: recovery keeps the last good state).
    fn save_state(&mut self) -> Option<Vec<u8>> {
        if self.shared.fault() == Fault::None
            && let Ok(mut g) = self.link.lock()
            && let Some(link) = g.as_mut()
            && link.stdin.write_all(&[4u8]).and_then(|()| link.stdin.flush()).is_ok()
            && let Some(r) = wait_ctl(link, 4)
        {
            *self.shared.last_state.lock().unwrap_or_else(PoisonError::into_inner) = Some(r.blob.clone());
            return Some(r.blob);
        }
        self.shared.last_state.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    fn load_state(&mut self, data: &[u8]) -> bool {
        if data.len() > MAX_STATE {
            return false;
        }
        *self.shared.last_state.lock().unwrap_or_else(PoisonError::into_inner) = Some(data.to_vec());
        let Ok(mut g) = self.link.lock() else { return false };
        let Some(link) = g.as_mut() else { return false };
        let mut req = Vec::with_capacity(5 + data.len());
        req.push(5u8);
        req.extend_from_slice(&(data.len() as u32).to_le_bytes());
        req.extend_from_slice(data);
        if link.stdin.write_all(&req).and_then(|()| link.stdin.flush()).is_err() {
            return false;
        }
        wait_ctl(link, 5).is_some_and(|r| r.ok)
    }
}

// ---- worker side --------------------------------------------------------------------------

/// The worker process: hosts `inner` in this process and serves the protocol on stdin/stdout.
/// `dirs` are extra plugin search folders (the host's). `SML_SANDBOX_FAULT=crash:<n>|hang:<n>`
/// injects a fault after `n` processed blocks (fault tests only). Returns the exit code.
pub fn worker_main(inner: &str, dirs: &[PathBuf]) -> i32 {
    let mut fault_spec = std::env::var("SML_SANDBOX_FAULT").ok();
    for d in dirs {
        if let Some(f) = d.to_str().and_then(|s| s.strip_prefix("--fault=")) {
            fault_spec = Some(f.to_string());
        } else if d.is_dir() {
            soundcraft_clap_host::add_search_dir(d);
            soundcraft_vst3_host::add_search_dir(d);
        }
    }
    let fault = fault_spec.and_then(|v| {
        let (kind, n) = v.split_once(':')?;
        Some((kind.to_string(), n.parse::<u64>().ok()?))
    });
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let (mut inp, mut out) = (stdin.lock(), stdout.lock());
    let mut plugin: Option<Box<dyn Plugin>> = None;
    let mut blocks = 0u64;
    let mut io: Vec<Vec<f32>> = Vec::new();
    let mut side: Vec<Vec<f32>> = Vec::new();
    let mut tag = [0u8; 1];
    loop {
        if inp.read_exact(&mut tag).is_err() {
            return 0;
        }
        match tag[0] {
            1 => {
                let mut b = [0u8; 12];
                if inp.read_exact(&mut b).is_err() {
                    return 0;
                }
                let sr = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                let mb = (u32::from_le_bytes([b[4], b[5], b[6], b[7]]) as usize).clamp(1, MAX_FRAMES);
                let ch = (u32::from_le_bytes([b[8], b[9], b[10], b[11]]) as usize).clamp(1, MAX_CHANNELS);
                if plugin.is_none() {
                    plugin = crate::create_instance(inner, None, sr, mb, ch);
                } else if let Some(p) = plugin.as_mut() {
                    p.prepare(sr, mb, ch);
                }
                io = vec![vec![0.0; mb]; ch];
                side = vec![vec![0.0; mb]; MAX_CHANNELS.min(8)];
                let mut r = [0u8; 11];
                r[0] = 1;
                if let Some(p) = plugin.as_ref() {
                    r[1] = 1;
                    r[2..6].copy_from_slice(&(p.latency() as u32).to_le_bytes());
                    r[6..10].copy_from_slice(&(p.tail_samples() as u32).to_le_bytes());
                    r[10] = u8::from(p.accepts_sidechain());
                }
                if out.write_all(&r).and_then(|()| out.flush()).is_err() {
                    return 0;
                }
            }
            2 => {
                let mut h = [0u8; 12];
                if inp.read_exact(&mut h).is_err() {
                    return 0;
                }
                let frames = u32::from_le_bytes([h[0], h[1], h[2], h[3]]) as usize;
                let ch = u32::from_le_bytes([h[4], h[5], h[6], h[7]]) as usize;
                let side_ch = u32::from_le_bytes([h[8], h[9], h[10], h[11]]) as usize;
                if frames > MAX_FRAMES || ch > MAX_CHANNELS || side_ch > MAX_CHANNELS {
                    return 3;
                }
                let mut bytes = vec![0u8; (ch + side_ch) * frames * 4];
                if inp.read_exact(&mut bytes).is_err() {
                    return 0;
                }
                if io.len() < ch || io.first().is_some_and(|c| c.len() < frames) {
                    io = vec![vec![0.0; frames.max(io.first().map_or(0, Vec::len))]; ch.max(io.len())];
                }
                if side.len() < side_ch || side.first().is_some_and(|c| c.len() < frames) {
                    side = vec![vec![0.0; frames.max(side.first().map_or(0, Vec::len))]; side_ch.max(side.len())];
                }
                let mut k = 0;
                for c in io.iter_mut().take(ch) {
                    for x in c.iter_mut().take(frames) {
                        let b = &bytes[k..k + 4];
                        *x = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                        k += 4;
                    }
                }
                for c in side.iter_mut().take(side_ch) {
                    for x in c.iter_mut().take(frames) {
                        let b = &bytes[k..k + 4];
                        *x = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                        k += 4;
                    }
                }
                blocks += 1;
                if let Some((kind, n)) = &fault
                    && blocks > *n
                {
                    match kind.as_str() {
                        "crash" => std::process::abort(),
                        "hang" => std::thread::sleep(Duration::from_secs(3600)),
                        _ => {}
                    }
                }
                let (mut latency, mut gr) = (0u32, 0.0f32);
                if let Some(p) = plugin.as_mut() {
                    if side_ch > 0 {
                        p.process_sidechain(&mut io[..ch], &side[..side_ch], frames);
                    } else {
                        p.process(&mut io[..ch], frames);
                    }
                    latency = p.latency() as u32;
                    gr = p.gain_reduction_db();
                }
                let mut r = Vec::with_capacity(17 + ch * frames * 4);
                r.push(2u8);
                r.extend_from_slice(&(frames as u32).to_le_bytes());
                r.extend_from_slice(&(ch as u32).to_le_bytes());
                for c in io.iter().take(ch) {
                    for x in c.iter().take(frames) {
                        r.extend_from_slice(&x.to_le_bytes());
                    }
                }
                r.extend_from_slice(&latency.to_le_bytes());
                r.extend_from_slice(&gr.to_le_bytes());
                if out.write_all(&r).and_then(|()| out.flush()).is_err() {
                    return 0;
                }
            }
            3 => {
                let mut l = [0u8; 2];
                if inp.read_exact(&mut l).is_err() {
                    return 0;
                }
                let mut id = vec![0u8; u16::from_le_bytes(l) as usize];
                let mut v = [0u8; 4];
                if inp.read_exact(&mut id).is_err() || inp.read_exact(&mut v).is_err() {
                    return 0;
                }
                if let (Some(p), Ok(id)) = (plugin.as_mut(), std::str::from_utf8(&id)) {
                    p.set_param(id, f32::from_le_bytes(v));
                }
            }
            4 => {
                let blob = plugin.as_mut().and_then(|p| p.save_state()).unwrap_or_default();
                let mut r = Vec::with_capacity(5 + blob.len());
                r.push(4u8);
                r.extend_from_slice(&(blob.len() as u32).to_le_bytes());
                r.extend_from_slice(&blob);
                if out.write_all(&r).and_then(|()| out.flush()).is_err() {
                    return 0;
                }
            }
            5 => {
                let mut l = [0u8; 4];
                if inp.read_exact(&mut l).is_err() {
                    return 0;
                }
                let len = u32::from_le_bytes(l) as usize;
                if len > MAX_STATE {
                    return 3;
                }
                let mut blob = vec![0u8; len];
                if inp.read_exact(&mut blob).is_err() {
                    return 0;
                }
                let ok = plugin.as_mut().is_some_and(|p| p.load_state(&blob));
                if out.write_all(&[5u8, u8::from(ok)]).and_then(|()| out.flush()).is_err() {
                    return 0;
                }
            }
            6 => {
                if let Some(p) = plugin.as_mut() {
                    p.reset();
                }
            }
            7 => {
                let mut b = [0u8; 6];
                if inp.read_exact(&mut b).is_err() {
                    return 0;
                }
                if let Some(p) = plugin.as_mut() {
                    p.note_on(u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize, b[4], b[5]);
                }
            }
            8 => {
                let mut b = [0u8; 5];
                if inp.read_exact(&mut b).is_err() {
                    return 0;
                }
                if let Some(p) = plugin.as_mut() {
                    p.note_off(u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize, b[4]);
                }
            }
            9 => {
                if let Some(p) = plugin.as_mut() {
                    p.all_notes_off();
                }
            }
            _ => return 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_defaults() {
        assert_eq!(inner_id("sandbox:clap:a.b"), Some("clap:a.b"));
        assert_eq!(inner_id("sandbox:"), None);
        assert_eq!(inner_id("clap:a.b"), None);
        assert!(auto_restart());
        assert!(create("sandbox:clap:does.not.exist").is_none(), "unknown inner id: no worker spawned");
    }
}
