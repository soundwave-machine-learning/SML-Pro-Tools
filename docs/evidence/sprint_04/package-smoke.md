# Package smoke (Sprint 4)

| Artifact | Check | Result |
|---|---|---|
| Linux tarball | clean extraction in a scratch folder; `soundcraft-cli --version`; `plugins` (empty catalogue); headless demo: `bus.create_from_selection`, `mix.width`, `mix.profile logic`, bounce | PASS (`package-smoke.log`) — CLI only |
| Linux tarball | launch `soundcraft` GUI | NOT POSSIBLE here: "neither WAYLAND_DISPLAY nor WAYLAND_SOCKET nor DISPLAY is set" (headless container). The same GUI code was rendered offscreen via `ui_shot` (Sprint 3 images). |
| Windows zip | DLL imports inspected (`x86_64-w64-mingw32-objdump -p`): only Windows system DLLs (dxgi, opengl32, ole32, uiautomationcore, …) | Self-contained |
| Windows zip | extraction/launch/setup/record/reopen/plugin/export on Windows, 1366×768 & 1920×1080 at 100/125/150 % | NOT RUN — no Windows host (H4 BLOCKED FOR TARGET VALIDATION) |

Hashes and sizes: `artifact-manifest.json`. Package/binary names keep the upstream `soundcraft`
names (crates and packaging ids were deliberately not renamed for branding; the window title,
app menu and About entry read "SML Recording").
