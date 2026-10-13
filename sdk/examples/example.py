"""SML Studio Python SDK example.

    python example.py PORT                    read-only: capabilities and the open session
    python example.py PORT --temporary-session  ALSO replaces the open session with a new,
                                                unsaved temporary one and adds two tracks

The second form refuses to run when the open session has unsaved changes (it would discard
them). Never point it at real work. Token: SOUNDCRAFT_CONTROL_TOKEN or the token file.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "python"))
import sml_studio  # noqa: E402

port = int(sys.argv[1])
with sml_studio.Client(port, client="sdk-example-python") as sml:
    caps = sml.capabilities()
    print(f"protocol {caps['protocol']}, app {caps['server']['app_version']}, {len(caps['commands'])} commands")
    s = sml.inspect()
    print(f"open session: {s.get('name')!r}, dirty={s.get('dirty')}, {len(s.get('tracks', []))} tracks")
    if "--temporary-session" in sys.argv:
        if s.get("dirty"):
            sys.exit("refused: the open session has unsaved changes (save it first, or run read-only)")
        sml.execute("session.new", {"name": "SDK example (temporary)"})
        sml.execute("track.new", {"count": 2, "format": "mono", "name": "SDK"})
        t = sml.inspect()
        print("temporary session tracks:", [x["name"] for x in t["tracks"]])
