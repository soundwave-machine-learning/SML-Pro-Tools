// End-to-end tests of the JavaScript SDK against a real authenticated server (SML_PORT).
import assert from "node:assert/strict";
import net from "node:net";
import { Client, AuthError, CommandError, ProtocolError, RequestTimeout } from "./sml-studio.mjs";

const PORT = Number(process.env.SML_PORT);
let failed = 0;
async function t(name, f) {
  try {
    await f();
    console.log(`ok   ${name}`);
  } catch (e) {
    failed++;
    console.log(`FAIL ${name}: ${e.stack ?? e}`);
  }
}

await t("capabilities, inspect, temporary session, typed errors", async () => {
  const c = new Client(PORT);
  try {
    const caps = await c.capabilities();
    assert.equal(caps.protocol, 2);
    assert.ok(caps.commands.includes("track.new"));
    await c.execute("session.new", { name: "SDK JS test (temporary)" });
    await c.execute("track.new", { count: 3, format: "mono" });
    assert.equal((await c.inspect()).tracks.length, 3);
    await assert.rejects(c.execute("no.such_command"), CommandError);
    await assert.rejects(c.scanStart(), (e) => e instanceof CommandError && /scan/i.test(e.message));
  } finally {
    c.close();
  }
});

await t("wrong token", async () => {
  const c = new Client(PORT, { token: "smlc_" + "0".repeat(64) });
  await assert.rejects(c.connect(), (e) => e instanceof AuthError && !e.message.includes("0".repeat(64)));
});

await t("timeout is typed and the request is not replayed", async () => {
  let got = 0;
  const srv = net.createServer((s) => s.on("data", () => got++));
  await new Promise((r) => srv.listen(0, "127.0.0.1", r));
  const c = new Client(srv.address().port, { token: "smlc_" + "a".repeat(64), timeoutMs: 300 });
  await assert.rejects(c.connect(), RequestTimeout);
  assert.equal(got, 1);
  srv.close();
});

await t("a protocol 1 (Hotfix 02) app is named", async () => {
  const srv = net.createServer((s) => s.once("data", () => s.end('{"id":1,"ok":false,"error":"unknown method `auth`"}\n')));
  await new Promise((r) => srv.listen(0, "127.0.0.1", r));
  const c = new Client(srv.address().port, { token: "smlc_" + "a".repeat(64) });
  await assert.rejects(c.connect(), (e) => e instanceof ProtocolError && /protocol 1/.test(e.message));
  srv.close();
});

await t("protocol errors are typed", async () => {
  assert.ok(new ProtocolError("x") instanceof Error);
});

process.exitCode = failed ? 1 : 0;
