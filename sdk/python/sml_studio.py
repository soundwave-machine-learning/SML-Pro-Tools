"""SML Studio local control client (Python, standard library only).

Talks to a running SML Studio (``soundcraft --control PORT``) or to ``soundcraft-cli serve`` over
the local control channel, protocol 2 (authenticated). Importing this module does nothing: no
connection, no command. Nothing is sent until you call a method.

Rules this client follows (see docs/control-protocol.md):

* The first message of a connection is ``auth`` with the token. The token comes from the
  ``token`` argument, else ``SOUNDCRAFT_CONTROL_TOKEN``, else the ``control-token`` file in the
  SoundCraft settings folder. The token is never logged or put in exception messages.
* Every request carries an id; replies are matched to it.
* A request that was sent is **never** re-sent automatically (it may already have run). If the
  connection breaks before the reply, :class:`ConnectionLost` is raised and you decide.
* Timeouts are per request (:class:`RequestTimeout`); a timeout does not cancel the request in
  the app.

Version: 1.0.0 (control protocol 2). Requires Python 3.8+.
"""

from __future__ import annotations

import json
import os
import socket
import sys
import time
from pathlib import Path
from typing import Any, Dict, Optional

__version__ = "1.0.0"
PROTOCOL = 2

__all__ = [
    "Client",
    "SmlError",
    "AuthError",
    "ProtocolError",
    "CommandError",
    "RequestTimeout",
    "ConnectionLost",
    "default_token_path",
    "PROTOCOL",
]


class SmlError(Exception):
    """Base class of every error raised by this client."""


class AuthError(SmlError):
    """The app refused the token (missing, wrong or revoked) or no token was found."""


class ProtocolError(SmlError):
    """The app speaks another protocol version, or replied with something unexpected."""


class CommandError(SmlError):
    """The app ran the request and refused it (``code`` is the app's error code, if any)."""

    def __init__(self, message: str, code: Optional[str] = None):
        super().__init__(message)
        self.code = code


class RequestTimeout(SmlError):
    """No reply in time. The request may still run in the app; check state before retrying."""


class ConnectionLost(SmlError):
    """The connection closed. If ``sent`` is true the request may have run."""

    def __init__(self, message: str, sent: bool):
        super().__init__(message)
        self.sent = sent


def default_token_path() -> Optional[Path]:
    """The token file the app writes (same rules as the app's settings folder)."""
    override = os.environ.get("SOUNDCRAFT_CONFIG_DIR")
    if override is not None:
        return Path(override) / "control-token" if override else None
    if sys.platform == "win32":
        base = os.environ.get("APPDATA")
        return Path(base) / "SoundCraft" / "control-token" if base else None
    if sys.platform == "darwin":
        home = os.environ.get("HOME")
        return Path(home) / "Library" / "Application Support" / "SoundCraft" / "control-token" if home else None
    xdg = os.environ.get("XDG_CONFIG_HOME") or (os.path.join(os.environ["HOME"], ".config") if os.environ.get("HOME") else None)
    return Path(xdg) / "soundcraft" / "control-token" if xdg else None


def _resolve_token(token: Optional[str]) -> Optional[str]:
    if token:
        return token.strip()
    env = os.environ.get("SOUNDCRAFT_CONTROL_TOKEN", "").strip()
    if env:
        return env
    p = default_token_path()
    if p and p.is_file():
        t = p.read_text(encoding="utf-8").strip()
        return t or None
    return None


class Client:
    """One authenticated connection to SML Studio's control channel.

    Example::

        with Client(7979) as sml:
            print(sml.capabilities()["protocol"])
            print(sml.inspect()["name"])
    """

    def __init__(self, port: int, host: str = "127.0.0.1", token: Optional[str] = None, client: str = "sml-python-sdk", timeout: float = 30.0):
        self.host, self.port = host, int(port)
        self._token = token
        self.client = client[:64]
        self.timeout = timeout
        self._sock: Optional[socket.socket] = None
        self._buf = b""
        self._next_id = 1
        self.server: Optional[Dict[str, Any]] = None

    # -- connection -------------------------------------------------------------------------
    def connect(self) -> "Client":
        """Opens the connection and authenticates. Raises AuthError / ProtocolError."""
        if self._sock is not None:
            return self
        token = _resolve_token(self._token)
        if not token:
            raise AuthError("no control token: pass token=, set SOUNDCRAFT_CONTROL_TOKEN, or start SML Studio with --control to create the token file")
        try:
            self._sock = socket.create_connection((self.host, self.port), timeout=5.0)
        except OSError as e:
            raise ConnectionLost(f"cannot reach SML Studio at {self.host}:{self.port}: {e}", sent=False) from None
        try:
            reply = self._exchange("auth", {"token": token, "client": self.client, "protocol": PROTOCOL}, self.timeout)
        except CommandError as e:
            self.close()
            if e.code in ("unauthorized", "auth_required"):
                raise AuthError("the app refused the control token (missing, wrong or revoked)") from None
            if e.code == "protocol":
                raise ProtocolError(str(e)) from None
            if "unknown method" in str(e) and "auth" in str(e):
                raise ProtocolError("this SML Studio speaks control protocol 1 (a build before Hotfix 03, without authentication); this SDK needs protocol 2") from None
            raise
        except SmlError:
            self.close()
            raise
        if not isinstance(reply, dict) or reply.get("protocol") != PROTOCOL:
            self.close()
            raise ProtocolError(f"unexpected protocol in the app's reply: {reply!r}")
        self.server = reply
        return self

    def close(self) -> None:
        if self._sock is not None:
            try:
                self._sock.close()
            finally:
                self._sock = None
                self._buf = b""

    def __enter__(self) -> "Client":
        return self.connect()

    def __exit__(self, *exc: Any) -> None:
        self.close()

    # -- requests ---------------------------------------------------------------------------
    def _exchange(self, method: str, params: Dict[str, Any], timeout: float) -> Any:
        assert self._sock is not None
        rid = self._next_id
        self._next_id += 1
        line = json.dumps({"id": rid, "method": method, "params": params}) + "\n"
        try:
            self._sock.sendall(line.encode("utf-8"))
        except OSError as e:
            self.close()
            raise ConnectionLost(f"could not send {method}: {e}", sent=False) from None
        deadline = time.monotonic() + timeout
        while b"\n" not in self._buf:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                self.close()
                raise RequestTimeout(f"{method}: no reply within {timeout:g} s (it may still run; check the state before retrying)")
            self._sock.settimeout(remaining)
            try:
                chunk = self._sock.recv(65536)
            except socket.timeout:
                continue
            except OSError as e:
                self.close()
                raise ConnectionLost(f"{method}: connection lost before the reply ({e}); it may have run", sent=True) from None
            if not chunk:
                self.close()
                raise ConnectionLost(f"{method}: the app closed the connection before replying; it may have run", sent=True)
            self._buf += chunk
        raw, self._buf = self._buf.split(b"\n", 1)
        try:
            msg = json.loads(raw)
        except ValueError:
            raise ProtocolError(f"{method}: the reply is not JSON") from None
        if msg.get("id") not in (rid, None):
            raise ProtocolError(f"{method}: reply id {msg.get('id')!r} does not match request id {rid}")
        if msg.get("ok"):
            return msg.get("result")
        raise CommandError(str(msg.get("error", "error")), msg.get("code"))

    def call(self, method: str, params: Optional[Dict[str, Any]] = None, timeout: Optional[float] = None) -> Any:
        """One control-protocol method (``engine.execute``, ``session.inspect``, ``ui.*``…)."""
        self.connect()
        return self._exchange(method, params or {}, self.timeout if timeout is None else timeout)

    def execute(self, command: str, params: Optional[Dict[str, Any]] = None, timeout: Optional[float] = None) -> Any:
        """Runs one registered command (see docs/reference/commands.json). One undo step."""
        return self.call("engine.execute", {"command": command, "params": params or {}}, timeout)

    # -- conveniences -----------------------------------------------------------------------
    def capabilities(self) -> Dict[str, Any]:
        """Protocol, app version and the commands this app has (read-only)."""
        self.connect()
        cmds = self.call("engine.commands", {})
        ids = [c.get("id") for c in cmds] if isinstance(cmds, list) else cmds
        return {"protocol": PROTOCOL, "sdk": __version__, "server": self.server, "commands": ids}

    def has_command(self, command: str) -> bool:
        cmds = self.capabilities()["commands"]
        return command in cmds if isinstance(cmds, list) else False

    def inspect(self) -> Dict[str, Any]:
        """The open session (read-only)."""
        return self.call("session.inspect", {})

    def scan_start(self, rescan: bool = False) -> Dict[str, Any]:
        """Starts or joins a plugin scan job; returns {job, coalesced}."""
        return self.execute("engine.plugin_scan_start", {"rescan": rescan})

    def scan_status(self, job: Optional[int] = None) -> Dict[str, Any]:
        return self.execute("engine.plugin_scan_status", {} if job is None else {"job": job})

    def scan_cancel(self, job: Optional[int] = None) -> Dict[str, Any]:
        return self.execute("engine.plugin_scan_cancel", {} if job is None else {"job": job})

    def scan_wait(self, job: int, timeout: float = 600.0, poll: float = 2.0) -> Dict[str, Any]:
        """Polls a scan job until it is done or cancelled, or raises RequestTimeout."""
        end = time.monotonic() + timeout
        while True:
            st = self.scan_status(job)
            state = (st.get("job") or st).get("state") if isinstance(st, dict) else None
            if state in ("done", "cancelled"):
                return st
            if time.monotonic() >= end:
                raise RequestTimeout(f"scan job {job} still {state} after {timeout:g} s (it keeps running)")
            time.sleep(poll)
