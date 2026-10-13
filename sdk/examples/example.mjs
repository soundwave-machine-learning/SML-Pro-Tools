// SML Studio JavaScript SDK example (Node 18+).
//   node example.mjs PORT                       read-only: capabilities and the open session
//   node example.mjs PORT --temporary-session   ALSO replaces the open session with a new,
//                                               unsaved temporary one and adds two tracks
// The second form refuses when the open session has unsaved changes. Never use it on real work.
import { Client } from "../js/sml-studio.mjs";

const port = Number(process.argv[2]);
const sml = new Client(port, { client: "sdk-example-js" });
try {
  const caps = await sml.capabilities();
  console.log(`protocol ${caps.protocol}, app ${caps.server.app_version}, ${caps.commands.length} commands`);
  const s = await sml.inspect();
  console.log(`open session: ${JSON.stringify(s.name)}, dirty=${s.dirty}, ${(s.tracks ?? []).length} tracks`);
  if (process.argv.includes("--temporary-session")) {
    if (s.dirty) {
      console.error("refused: the open session has unsaved changes (save it first, or run read-only)");
      process.exitCode = 1;
    } else {
      await sml.execute("session.new", { name: "SDK example (temporary)" });
      await sml.execute("track.new", { count: 2, format: "mono", name: "SDK" });
      const t = await sml.inspect();
      console.log("temporary session tracks:", t.tracks.map((x) => x.name));
    }
  }
} finally {
  sml.close();
}
