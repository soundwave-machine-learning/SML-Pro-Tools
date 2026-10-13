// SML Studio local control client (JavaScript, Node.js 18+, no dependencies).
//
// Talks to a running SML Studio (`soundcraft --control PORT`) or `soundcraft-cli serve` over the
// local control channel, protocol 2 (authenticated). Importing this module does nothing: no
// connection, no command.
//
// Rules (docs/control-protocol.md): the first message is `auth` with the token (from the
// `token` option, else SOUNDCRAFT_CONTROL_TOKEN, else the `control-token` file in the SoundCraft
// settings folder); every request carries an id; a request that was sent is never re-sent
// automatically; a timeout does not cancel the request in the app. The token is never logged.
//
// Version 1.0.0 (control protocol 2).

import net from "node:net";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

export const VERSION = "1.0.0";
export const PROTOCOL = 2;

export class SmlError extends Error {}
/** The app refused the token (missing, wrong or revoked), or no token was found. */
export class AuthError extends SmlError {}
/** Another protocol version, or an unexpected reply. */
export class ProtocolError extends SmlError {}
/** The app ran the request and refused it; `code` is the app's error code. */
export class CommandError extends SmlError {
  constructor(message, code) {
    super(message);
    this.code = code ?? null;
  }
}
/** No reply in time; the request may still run in the app. */
export class RequestTimeout extends SmlError {}
/** The connection closed; when `sent` is true the request may have run. */
export class ConnectionLost extends SmlError {
  constructor(message, sent) {
    super(message);
    this.sent = sent;
  }
}

/** The token file the app writes (same rules as its settings folder). */
export function defaultTokenPath() {
  const o = process.env.SOUNDCRAFT_CONFIG_DIR;
  if (o !== undefined) return o ? path.join(o, "control-token") : null;
  if (process.platform === "win32") return process.env.APPDATA ? path.join(process.env.APPDATA, "SoundCraft", "control-token") : null;
  if (process.platform === "darwin") return path.join(os.homedir(), "Library", "Application Support", "SoundCraft", "control-token");
  const xdg = process.env.XDG_CONFIG_HOME || path.join(os.homedir(), ".config");
  return path.join(xdg, "soundcraft", "control-token");
}

function resolveToken(token) {
  if (token) return token.trim();
  const env = (process.env.SOUNDCRAFT_CONTROL_TOKEN || "").trim();
  if (env) return env;
  const p = defaultTokenPath();
  try {
    return p ? fs.readFileSync(p, "utf8").trim() || null : null;
  } catch {
    return null;
  }
}

export class Client {
  /**
   * @param {number} port
   * @param {{host?: string, token?: string, client?: string, timeoutMs?: number}} [opts]
   */
  constructor(port, opts = {}) {
    this.port = port;
    this.host = opts.host ?? "127.0.0.1";
    this._token = opts.token;
    this.client = (opts.client ?? "sml-js-sdk").slice(0, 64);
    this.timeoutMs = opts.timeoutMs ?? 30000;
    this._sock = null;
    this._buf = "";
    this._nextId = 1;
    this._waiter = null;
    this.server = null;
  }

  /** Opens the connection and authenticates. */
  async connect() {
    if (this._sock) return this;
    const token = resolveToken(this._token);
    if (!token) throw new AuthError("no control token: pass {token}, set SOUNDCRAFT_CONTROL_TOKEN, or start SML Studio with --control to create the token file");
    await new Promise((resolve, reject) => {
      const s = net.createConnection({ host: this.host, port: this.port }, () => resolve());
      s.setEncoding("utf8");
      s.once("error", (e) => reject(new ConnectionLost(`cannot reach SML Studio at ${this.host}:${this.port}: ${e.message}`, false)));
      s.on("data", (d) => this._onData(d));
      s.on("close", () => this._onClose());
      this._sock = s;
    }).catch((e) => {
      this._sock = null;
      throw e;
    });
    try {
      const reply = await this._exchange("auth", { token, client: this.client, protocol: PROTOCOL }, this.timeoutMs);
      if (!reply || reply.protocol !== PROTOCOL) throw new ProtocolError(`unexpected protocol in the app's reply: ${JSON.stringify(reply)}`);
      this.server = reply;
    } catch (e) {
      this.close();
      if (e instanceof CommandError && (e.code === "unauthorized" || e.code === "auth_required")) throw new AuthError("the app refused the control token (missing, wrong or revoked)");
      if (e instanceof CommandError && e.code === "protocol") throw new ProtocolError(e.message);
      if (e instanceof CommandError && /unknown method/.test(e.message) && /auth/.test(e.message))
        throw new ProtocolError("this SML Studio speaks control protocol 1 (a build before Hotfix 03, without authentication); this SDK needs protocol 2");
      throw e;
    }
    return this;
  }

  close() {
    if (this._sock) {
      this._sock.destroy();
      this._sock = null;
    }
  }

  _onData(d) {
    this._buf += d;
    let i;
    while ((i = this._buf.indexOf("\n")) >= 0) {
      const line = this._buf.slice(0, i);
      this._buf = this._buf.slice(i + 1);
      const w = this._waiter;
      this._waiter = null;
      if (w) w.line(line);
    }
  }

  _onClose() {
    const w = this._waiter;
    this._waiter = null;
    this._sock = null;
    if (w) w.closed();
  }

  _exchange(method, params, timeoutMs) {
    const id = this._nextId++;
    return new Promise((resolve, reject) => {
      if (!this._sock) return reject(new ConnectionLost(`could not send ${method}: not connected`, false));
      const timer = setTimeout(() => {
        this._waiter = null;
        this.close();
        reject(new RequestTimeout(`${method}: no reply within ${timeoutMs} ms (it may still run; check the state before retrying)`));
      }, timeoutMs);
      this._waiter = {
        line: (line) => {
          clearTimeout(timer);
          let msg;
          try {
            msg = JSON.parse(line);
          } catch {
            return reject(new ProtocolError(`${method}: the reply is not JSON`));
          }
          if (msg.id !== id && msg.id !== null && msg.id !== undefined) return reject(new ProtocolError(`${method}: reply id ${msg.id} does not match ${id}`));
          if (msg.ok) resolve(msg.result);
          else reject(new CommandError(String(msg.error ?? "error"), msg.code));
        },
        closed: () => {
          clearTimeout(timer);
          reject(new ConnectionLost(`${method}: the app closed the connection before replying; it may have run`, true));
        },
      };
      this._sock.write(JSON.stringify({ id, method, params }) + "\n");
    });
  }

  /** One control-protocol method. */
  async call(method, params = {}, timeoutMs) {
    await this.connect();
    return this._exchange(method, params, timeoutMs ?? this.timeoutMs);
  }

  /** Runs one registered command (docs/reference/commands.json). One undo step. */
  execute(command, params = {}, timeoutMs) {
    return this.call("engine.execute", { command, params }, timeoutMs);
  }

  /** Protocol, app version and the commands this app has (read-only). */
  async capabilities() {
    await this.connect();
    const cmds = await this.call("engine.commands", {});
    const ids = Array.isArray(cmds) ? cmds.map((c) => c.id) : cmds;
    return { protocol: PROTOCOL, sdk: VERSION, server: this.server, commands: ids };
  }

  /** The open session (read-only). */
  inspect() {
    return this.call("session.inspect", {});
  }

  scanStart(rescan = false) {
    return this.execute("engine.plugin_scan_start", { rescan });
  }
  scanStatus(job) {
    return this.execute("engine.plugin_scan_status", job === undefined ? {} : { job });
  }
  scanCancel(job) {
    return this.execute("engine.plugin_scan_cancel", job === undefined ? {} : { job });
  }
}
