//! E03 runtime-isolation experiment (NOT a product feature): measures what moving one plugin
//! into a worker process costs on this machine with the only IPC available without `unsafe`
//! (pipes), and how a crashed or hung worker is survived.
//!
//! For each block size the host sends 60 s of blocks to `soundcraft-cli plugin-worker-probe`
//! (a gain "plugin") and records the round-trip per block: the added latency a callback would
//! pay if it *waited* for the worker (which it must never do), and how many blocks exceed the
//! callback budget. Then a worker is killed mid-stream and another made to hang: the host must
//! detect both without blocking and switch to a fault state (silence) on the next block.
//!
//! `cargo run --release -p soundcraft-engine --example ipc_probe -- out.csv`

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

struct Worker {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    /// Replies arrive through a reader thread; the host never blocks on the pipe itself.
    replies: Receiver<Vec<u8>>,
}

fn cli() -> std::path::PathBuf {
    let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("../soundcraft-cli")));
    exe.filter(|p| p.exists()).unwrap_or_else(|| std::path::PathBuf::from("soundcraft-cli"))
}

fn spawn(extra: &[&str]) -> Result<Worker, String> {
    let mut child = Command::new(cli())
        .arg("plugin-worker-probe")
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let stdin = child.stdin.take().ok_or("no stdin")?;
    let mut stdout = child.stdout.take().ok_or("no stdout")?;
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let mut head = [0u8; 4];
        loop {
            if stdout.read_exact(&mut head).is_err() {
                return;
            }
            let n = u32::from_le_bytes(head) as usize;
            let mut buf = vec![0u8; n * 4];
            if stdout.read_exact(&mut buf).is_err() || tx.send(buf).is_err() {
                return;
            }
        }
    });
    Ok(Worker { child, stdin, replies: rx })
}

/// One block round trip with a deadline: `Ok(micros)` or the fault that was detected.
fn round_trip(w: &mut Worker, block: &[u8], deadline: Duration) -> Result<f64, &'static str> {
    let t0 = Instant::now();
    let n = (block.len() / 4) as u32;
    if w.stdin.write_all(&n.to_le_bytes()).is_err() || w.stdin.write_all(block).is_err() || w.stdin.flush().is_err() {
        return Err("worker gone (write failed)");
    }
    match w.replies.recv_timeout(deadline) {
        Ok(reply) if reply.len() == block.len() => Ok(t0.elapsed().as_secs_f64() * 1e6),
        Ok(_) => Err("short reply"),
        Err(RecvTimeoutError::Timeout) => Err("deadline missed: worker hung or too slow"),
        Err(RecvTimeoutError::Disconnected) => Err("worker gone (pipe closed)"),
    }
}

fn main() {
    let out_path = std::env::args().nth(1).unwrap_or_else(|| "ipc-probe.csv".into());
    let sr = 48_000.0f64;
    let mut csv = String::from("case,frames,blocks,budget_us,p50_us,p99_us,max_us,over_budget,p99_added_latency_frames,outcome\n");
    for n in [64usize, 128, 256] {
        let Ok(mut w) = spawn(&[]) else {
            println!("cannot spawn the worker (build soundcraft-cli first)");
            return;
        };
        let block: Vec<u8> = (0..n).flat_map(|i| ((i as f32) / n as f32).to_le_bytes()).collect();
        let blocks = (sr as usize * 60) / n;
        let budget = Duration::from_secs_f64(n as f64 / sr);
        let mut times = Vec::with_capacity(blocks);
        let mut faults = 0usize;
        for _ in 0..blocks {
            match round_trip(&mut w, &block, budget * 4) {
                Ok(us) => times.push(us),
                Err(_) => faults += 1,
            }
        }
        let _ = w.child.kill();
        times.sort_by(f64::total_cmp);
        let pct = |p: f64| times.get(((times.len() as f64 - 1.0) * p) as usize).copied().unwrap_or(f64::NAN);
        let budget_us = budget.as_secs_f64() * 1e6;
        let over = times.iter().filter(|t| **t >= budget_us).count() + faults;
        let _ = std::fmt::Write::write_fmt(
            &mut csv,
            format_args!(
                "pipe round trip (gain worker),{n},{blocks},{budget_us:.1},{:.1},{:.1},{:.1},{over},{:.2},{}\n",
                pct(0.5),
                pct(0.99),
                times.last().copied().unwrap_or(f64::NAN),
                pct(0.99) / 1e6 * sr,
                if faults == 0 { "completed" } else { "faults during run" }
            ),
        );
    }
    // Fault cases at 128 frames: the worker dies after 100 blocks / stops answering after 100.
    for (case, extra) in
        [("worker crash (SIGABRT after 100 blocks)", ["--crash-after", "100"]), ("worker hang (silent after 100 blocks)", ["--hang-after", "100"])]
    {
        let Ok(mut w) = spawn(&extra) else { continue };
        let n = 128usize;
        let block: Vec<u8> = vec![0u8; n * 4];
        let budget = Duration::from_secs_f64(n as f64 / sr);
        let mut ok = 0usize;
        let mut outcome = String::from("no fault detected");
        let t0 = Instant::now();
        for _ in 0..1000 {
            match round_trip(&mut w, &block, budget * 4) {
                Ok(_) => ok += 1,
                Err(e) => {
                    outcome = format!(
                        "fault after {ok} blocks: {e}; detected in {:.1} ms; host continues (silence policy)",
                        t0.elapsed().as_secs_f64() * 1e3
                    );
                    break;
                }
            }
        }
        let _ = w.child.kill();
        let _ = w.child.wait();
        let _ = std::fmt::Write::write_fmt(&mut csv, format_args!("{case},{n},{ok},{:.1},,,,,,{outcome}\n", budget.as_secs_f64() * 1e6));
    }
    if let Err(e) = std::fs::write(&out_path, &csv) {
        println!("{out_path}: {e}");
    }
    print!("{csv}");
}
