"""End-to-end tests of the Python SDK against a real authenticated server.

Run by apps/soundcraft-cli/tests/sdk_e2e.rs with SML_PORT set (a `soundcraft-cli serve` with its
settings folder in SOUNDCRAFT_CONFIG_DIR). Standard library only.
"""
import importlib
import os
import socket
import sys
import threading
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

PORT = int(os.environ["SML_PORT"])


class Sdk(unittest.TestCase):
    def test_import_does_nothing(self):
        with mock.patch("socket.create_connection", side_effect=AssertionError("connected on import")):
            mod = importlib.reload(importlib.import_module("sml_studio"))
            mod.Client(PORT)  # constructing does not connect either

    def test_capabilities_inspect_and_temporary_session(self):
        import sml_studio
        with sml_studio.Client(PORT) as c:
            caps = c.capabilities()
            self.assertEqual(caps["protocol"], 2)
            self.assertIn("track.new", caps["commands"])
            self.assertIn("engine.plugin_scan_start", caps["commands"])
            self.assertEqual(c.server["protocol"], 2)
            c.execute("session.new", {"name": "SDK test (temporary)"})
            c.execute("track.new", {"count": 2, "format": "mono"})
            self.assertEqual(len(c.inspect()["tracks"]), 2)
            with self.assertRaises(sml_studio.CommandError):
                c.execute("no.such_command")
            # Scan jobs: the headless server has no scan service, so a typed error comes back.
            with self.assertRaises(sml_studio.CommandError) as e:
                c.scan_start()
            self.assertIn("scan", str(e.exception).lower())

    def test_missing_and_wrong_token(self):
        import sml_studio
        with self.assertRaises(sml_studio.AuthError) as e:
            sml_studio.Client(PORT, token="smlc_" + "0" * 64).connect()
        self.assertNotIn("0" * 64, str(e.exception))
        env = dict(os.environ)
        try:
            os.environ["SOUNDCRAFT_CONFIG_DIR"] = str(Path(env["SOUNDCRAFT_CONFIG_DIR"]) / "nowhere")
            os.environ.pop("SOUNDCRAFT_CONTROL_TOKEN", None)
            with self.assertRaises(sml_studio.AuthError):
                sml_studio.Client(PORT).connect()
        finally:
            os.environ.clear()
            os.environ.update(env)

    def test_newer_protocol_is_refused(self):
        import sml_studio
        with mock.patch.object(sml_studio, "PROTOCOL", 99):
            with self.assertRaises(sml_studio.ProtocolError):
                sml_studio.Client(PORT).connect()

    def test_timeout_is_typed_and_not_retried(self):
        import sml_studio
        srv = socket.socket()
        srv.bind(("127.0.0.1", 0))
        srv.listen(1)
        seen = []

        def silent():
            conn, _ = srv.accept()
            seen.append(conn.recv(65536))
            threading.Event().wait(2)
            conn.close()

        threading.Thread(target=silent, daemon=True).start()
        c = sml_studio.Client(srv.getsockname()[1], token="smlc_" + "a" * 64, timeout=0.5)
        with self.assertRaises(sml_studio.RequestTimeout):
            c.connect()
        self.assertEqual(len(seen), 1, "one request sent, never replayed")
        srv.close()


if __name__ == "__main__":
    unittest.main(verbosity=2)
