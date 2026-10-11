//! Prints the real input/output device capabilities this build can see, as JSON (CL-T2 audit).
//! `cargo run -p soundcraft-playback --example device_audit`
fn main() {
    use soundcraft_playback::device::{Direction, enumerate};
    let report = serde_json::json!({
        "hosts_compiled": cpal_hosts(),
        "inputs": enumerate(Direction::Input),
        "outputs": enumerate(Direction::Output),
        "asio_feature_compiled": false,
        "note": "cpal reports no clock-lock state; ClockLock is Unknown for every device",
    });
    println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
}

fn cpal_hosts() -> Vec<String> {
    cpal::available_hosts().into_iter().map(|h| h.name().to_string()).collect()
}
