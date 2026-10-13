//! Hotfix Pass 03: portable checks of HF15–HF22 behaviour reachable from the app layer.
use serde_json::json;
use soundcraft_ui_egui::{Services, SoundApp};

/// HF18: while a take is being recorded, every app entry that replaces the session or recovers
/// over it is refused — the commands (menus, shortcuts, control channel, assistant all run
/// through `SoundApp::run`), Recover Autosave's "Open as unsaved session" and "Recover to a new
/// track". The refusal changes nothing: the session, its undo history, the dirty flag and the
/// recording state stay as they were. After the take is stopped, recovery works.
#[test]
fn hf18_recording_blocks_every_session_replacement_entry() {
    let dir = std::env::temp_dir().join(format!("sml-hf18-app-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.autosave_dir = Some(dir.join("autosave"));
    let saved = dir.join("demo.scraft");
    app.run("session.save_as", json!({"path": saved.to_string_lossy()})).unwrap();
    app.run("mix.volume", json!({"track": "Kick", "db": -9.0})).unwrap();
    assert_eq!(app.recovery_snapshot_now().unwrap().map(|_| ()), Some(()));
    let point = app.recovery_points().into_iter().next().expect("a recovery point");
    // A capture journal that looks unfinished (as the take being written does).
    let media = dir.join("Take_01.wav");
    std::fs::write(&media, b"RIFF").unwrap();

    app.engine.transport.recording = true;
    let doc = app.engine.session_arc();
    let (rev, depth, dirty) = (app.engine.revision, app.engine.undo_depth(), app.engine.is_dirty());
    for (id, p) in [
        ("session.new", json!({})),
        ("session.open", json!({"path": saved.to_string_lossy()})),
        ("session.close", json!({})),
        ("session.revert", json!({})),
    ] {
        let err = app.run(id, p).unwrap_err();
        assert!(err.contains("refused while recording"), "{id}: {err}");
    }
    let err = app.open_recovery_point(&point).unwrap_err();
    assert!(err.contains("refused while recording"), "{err}");
    let take = soundcraft_playback::capture::UnfinishedTake {
        media: media.clone(),
        journal: serde_json::from_value(json!({"media": media.to_string_lossy(), "sample_rate": 48000, "channels": 1, "durable_frames": 0, "state": "Recording", "generation": null, "first_stream_frame": null, "error": null})).unwrap(),
    };
    let err = app.recover_take(&take).unwrap_err();
    assert!(err.contains("refused while recording"), "{err}");
    assert!(std::sync::Arc::ptr_eq(&doc, &app.engine.session_arc()), "session unchanged");
    assert_eq!((app.engine.revision, app.engine.undo_depth(), app.engine.is_dirty(), app.engine.transport.recording), (rev, depth, dirty, true));
    assert_eq!(std::fs::read(&media).unwrap(), b"RIFF", "the file being written is untouched");

    // Finished: deliberate recovery opens the recovery point as an unsaved session.
    app.engine.transport.recording = false;
    app.open_recovery_point(&point).unwrap();
    assert!(app.engine.path.is_none() && app.engine.is_dirty());
    assert_eq!(app.engine.session().track_by_name("Kick").unwrap().mixer.volume_db, -9.0);
    let _ = std::fs::remove_dir_all(&dir);
}
