# B03 — keyboard location (numpad vs main row) and Win/Super modifier: backend inspection

Inspected 2026-10-12 (environment P; no physical keyboard test possible here).

| Item | Finding | Source |
|---|---|---|
| Toolkit versions | egui 0.36.2, egui-winit 0.36.2, eframe 0.36 (workspace `Cargo.toml`) | `Cargo.lock` |
| Key translation | `key_from_key_code`: `KeyCode::Minus \| KeyCode::NumpadSubtract => Key::Minus`, `KeyCode::NumpadAdd => Key::Plus`, `KeyCode::Digit0 \| KeyCode::Numpad0 => Key::Num0` (…9), `Enter \| NumpadEnter => Key::Enter` | `/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/egui-winit-0.36.2/src/lib.rs` lines 1562, 1563, 1566 |
| Key location | the winit `KeyEvent.location` field is explicitly discarded (`location: _, // e.g. is it on the numpad?`) | `/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/egui-winit-0.36.2/src/lib.rs` line 991 |
| egui key set | `egui::Key` has no keypad variants; `Event::Key { key, physical_key, .. }` carries only translated keys | `/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/egui-0.36.2/src/data/key.rs` |
| Super/Win modifier | `Modifiers.mac_cmd = macos && super_key`, `command = ctrl` on other OSes: the Windows key is dropped on Windows/Linux | `/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/egui-winit-0.36.2/src/lib.rs` lines 495–496 |
| eframe hooks | `App::raw_input_hook` receives egui `RawInput` (already translated); no winit event hook without a custom integration | eframe 0.36 `App` trait |

## Consequence (status: BLOCKED at the toolkit, fallback in place)
- The app cannot tell keypad +/− from main-row +/−, and cannot see the Win key, through this
  toolkit version. Current behaviour (unchanged, from the prior cycle): plain +/− act as
  Num +/− only when no text field, plugin editor, modal or shortcut capture has focus
  (`crates/ui-egui/src/nudge_keys.rs`), which is a documented approximation, not key-location
  semantics.
- Remappable fallback: the Nudge actions are ordinary commands (`nudge.apply`, `nudge.set`,
  targets) and can be bound to any chord in Setup › Keyboard Shortcuts; a profile may bind
  Ctrl+Alt+Plus/Minus for the content-slip variant that Pro Tools puts on Win+Num±.
- Closure path: (a) a toolkit that forwards `KeyLocation` (egui-winit change or a custom
  eframe integration), (b) a physical-keyboard test on the target machine (TARGET_PENDING).
  Neither is portable work in this container.
