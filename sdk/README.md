# SML Studio SDKs (local)

Small clients for SML Studio's **local control channel, protocol 2**. They are local deliverables and are not published to any package registry. Both use only the standard library.

| SDK | File | Needs |
|---|---|---|
| Python | `python/sml_studio.py` | Python 3.8+ |
| JavaScript | `js/sml-studio.mjs` | Node.js 18+ |

The wire format is in `schema/sml-control-v2.schema.json` and `docs/control-protocol.md`. The command list is in `docs/reference/commands.json`, or ask a running app with `capabilities()`.

## Before you start

1. Start SML Studio with control enabled: `soundcraft --control 7979`. Or, for scripts and tests with no window or audio, run `soundcraft-cli serve --port 7979`.
2. The app creates a token file named `control-token` in its settings folder. On Windows that is `%APPDATA%\SoundCraft\control-token`. To find it, run `soundcraft-cli token path`.
3. The SDKs read the token from the `token` argument. If none is given, they read `SOUNDCRAFT_CONTROL_TOKEN`. If that is not set either, they read the token file.
4. **The token is a password.** Never print it, commit it or paste it into a chat. To rotate it: `soundcraft-cli token rotate` (or Setup › External Control… › Rotate). To revoke it: `soundcraft-cli token revoke`.

## Python

```python
import sml_studio
with sml_studio.Client(7979) as sml:
    print(sml.capabilities()["protocol"])     # 2
    print(sml.inspect()["name"])              # read-only
```

## JavaScript

```js
import { Client } from "./js/sml-studio.mjs";
const sml = new Client(7979);
console.log((await sml.capabilities()).protocol);
sml.close();
```

## API (both SDKs)

| Python | JavaScript | What |
|---|---|---|
| `Client(port, host, token, client, timeout)` | `new Client(port, {host, token, client, timeoutMs})` | Nothing is sent until the first call |
| `connect()` / `close()` / `with` | `connect()` / `close()` | Authenticates (`auth`) |
| `call(method, params)` | `call(method, params)` | Any protocol method |
| `execute(command, params)` | `execute(command, params)` | One registered command = one undo step |
| `capabilities()`, `has_command()` | `capabilities()` | Read-only |
| `inspect()` | `inspect()` | Read-only |
| `scan_start/status/cancel/wait` | `scanStart/scanStatus/scanCancel` | Plugin scan jobs (desktop app only) |

Errors are typed:

| Error | Meaning |
|---|---|
| `AuthError` | No token was found, or the token is wrong or revoked |
| `ProtocolError` | The app speaks a different protocol version |
| `CommandError(code)` | The app refused the request |
| `RequestTimeout` | No reply in time. The request may still run |
| `ConnectionLost(sent)` | The connection closed. If `sent` is true, the request may have run |

## Safety rules

- **No automatic retries.** A request that was sent is never sent again by the SDK, because it may already have run. After a timeout or a lost connection, call `inspect()` to check what happened, then decide.
- **Read-only first.** The examples only read unless you pass `--temporary-session`. That mode replaces the open session with a new, unsaved one. It refuses to run if the open session has unsaved changes.
- Run scripts that change things against `soundcraft-cli serve`, or against a disposable session. Never run them against real work.
- Do not script **Recover Autosave** or plugin scans against a production project.
- While recording, the app refuses session replacement (`session.new`, `session.open`, `session.close`, `session.revert`). Stop recording first.

## Examples and tests

```
python3 examples/example.py 7979                      # read-only
node examples/example.mjs 7979 --temporary-session    # disposable session only
cargo test -p soundcraft-cli --test sdk_e2e           # both SDKs against a real `serve`
```

The end-to-end test starts `soundcraft-cli serve` with a temporary settings folder, then checks:

- the SDK suites and the examples;
- wrong and missing tokens;
- a protocol version mismatch;
- a timeout that is never replayed;
- the refusal on a dirty session.
