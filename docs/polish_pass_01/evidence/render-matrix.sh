#!/usr/bin/env bash
# usage: render-matrix.sh OUTDIR — renders the polish-pass matrix (Record/Edit/Mix Console,
# Visual, Split/Master at 1920x1080, 1366x768, 911x512 = 1366x768 at 150 %, 960x540 at 200 %,
# plus Audio Setup, Assistant, New Tracks and Bounce) with the current build's ui_shot example.
set -u
OUT=$1; mkdir -p "$OUT/scripts"; cd "$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
cargo build -q -p soundcraft-ui-egui --example ui_shot 2>&1 | tail -3
BIN=target/debug/examples/ui_shot
mk(){ name=$1; size=$2; scale=$3; shift 3; f="$OUT/scripts/$name.jsonl"; : > "$f"; for c in "$@"; do echo "{\"method\":\"engine.execute\",\"params\":$c}" >> "$f"; done; echo '{"steps":8}' >> "$f"; $BIN "$OUT/$name.png" "$f" --size $size --scale $scale >/dev/null 2>&1 && echo "ok $name" || echo "FAIL $name"; }
E='{"command":"window.edit","params":{}}'; M='{"command":"window.mix","params":{}}'; R='{"command":"window.record","params":{}}'; MS='{"command":"window.master","params":{}}'
LOOP='{"command":"loop.set","params":{"start":96000,"end":384000}}'; LOOPON='{"command":"options.loop_playback","params":{"value":true}}'
SEL='{"command":"edit.select","params":{"tracks":["Snare"],"start":200000,"end":300000}}'
ARM='{"command":"mix.record_arm","params":{"tracks":["Kick"],"value":true}}'; MUTE='{"command":"mix.mute","params":{"tracks":["Hats"],"value":true}}'; SOLO='{"command":"mix.solo","params":{"tracks":["Bass"],"value":true}}'
for S in "1920x1080 1 desk" "911x512 1 compact150" "960x540 2 hidpi200" "1366x768 1 laptop"; do set -- $S; sz=$1; sc=$2; tag=$3
  mk edit-$tag $sz $sc "$E" "$LOOP" "$LOOPON" "$SEL" "$ARM" "$MUTE" "$SOLO"
  mk record-$tag $sz $sc "$R" "$ARM"
  mk mix-console-$tag $sz $sc "$M" '{"command":"mix.view","params":{"view":0}}' "$ARM" "$MUTE" "$SOLO" '{"command":"edit.select","params":{"tracks":["Snare"]}}'
  mk mix-visual-$tag $sz $sc "$M" '{"command":"mix.view","params":{"view":1}}'
  mk mix-split-$tag $sz $sc "$M" '{"command":"mix.view","params":{"view":2}}' "$MUTE" "$SOLO"
  mk master-$tag $sz $sc "$MS"
done
mk audio-setup-desk 1920x1080 1 "$E" '{"command":"edit.select","params":{"tracks":["Kick"]}}' "$ARM" '{"command":"window.audio_setup","params":{"value":true}}'
mk audio-setup-compact150 911x512 1 "$E" '{"command":"edit.select","params":{"tracks":["Kick"]}}' '{"command":"window.audio_setup","params":{"value":true}}'
mk assistant-desk 1920x1080 1 "$E" '{"command":"window.assistant","params":{}}'
mkraw(){ name=$1; size=$2; scale=$3; shift 3; f="$OUT/scripts/$name.jsonl"; : > "$f"; for c in "$@"; do echo "$c" >> "$f"; done; echo '{"steps":8}' >> "$f"; $BIN "$OUT/$name.png" "$f" --size $size --scale $scale >/dev/null 2>&1 && echo "ok $name" || echo "FAIL $name"; }
mkraw newtracks-compact150 911x512 1 '{"method":"engine.execute","params":{"command":"window.edit","params":{}}}' '{"method":"ui.menu.invoke","params":{"id":"track.new","path":"Track > New..."}}'
mkraw bounce-desk 1920x1080 1 '{"method":"engine.execute","params":{"command":"window.edit","params":{}}}' '{"method":"ui.menu.invoke","params":{"id":"ui.bounce_dialog","path":"File > Bounce Mix..."}}'
