#!/bin/bash
# Hotfix 03 update-script test with fake installs (PowerShell 7 on Linux; NOT Windows).
#   update_script_test.sh PWSH NEWZIP OLDZIP WORKDIR
# Old install = OLDZIP unpacked + a user session folder; fake settings with ui.json and a fake
# control-token (never a real one). Checks: checksum refusal, app-running refusal, -WhatIf,
# update (session kept, settings copied, token copy owner-only and never printed), exact rollback.
set -u
PW=$1; NEWZIP=$2; OLDZIP=$3; T=$4
R=$(cd "$(dirname "$0")/../../.." && pwd)
rm -rf "$T"; mkdir -p "$T/appdata/SoundCraft"
cp "$R/packaging/windows/Update-SMLStudioCandidate.ps1" "$T/"
cp "$NEWZIP" "$NEWZIP.sha256" "$T/"
(cd "$T" && unzip -q "$OLDZIP" && mv soundcraft-0.5.0-windows-x64-gnu-portable "SML Studio")
mkdir -p "$T/SML Studio/My Sessions" && echo '{"session":"keep me"}' > "$T/SML Studio/My Sessions/song.scraft"
echo '{"theme":"graphite"}' > "$T/appdata/SoundCraft/ui.json"
FAKE="smlc_$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n')"
( umask 077; printf '%s' "$FAKE" > "$T/appdata/SoundCraft/control-token" )
OLDHASH=$(sha256sum "$T/SML Studio/soundcraft.exe" | cut -d' ' -f1)
export APPDATA=$T/appdata
Z=$T/$(basename "$NEWZIP")
OUT=$T/all-output.txt; : > "$OUT"
run(){ "$PW" -NoProfile -File "$T/Update-SMLStudioCandidate.ps1" "$@" 2>&1 | tee -a "$OUT" | sed "s#$T#<test>#g"; }
log(){ echo; echo "### $1"; }
same(){ [ "$(sha256sum "$1" | cut -d' ' -f1)" = "$2" ] && echo yes || echo NO; }
echo "## Update-SMLStudioCandidate.ps1 test — $(date -u +%FT%TZ) — PowerShell $("$PW" -NoProfile -Command '$PSVersionTable.PSVersion.ToString()') on Linux (NOT Windows)"
echo "script sha256 $(sha256sum "$R/packaging/windows/Update-SMLStudioCandidate.ps1" | cut -d' ' -f1)"
echo "new package $(basename "$NEWZIP") sha256 $(cut -d' ' -f1 "$NEWZIP.sha256")"
echo "old install = Hotfix 02 candidate ($(basename "$(dirname "$OLDZIP")")) + a user session folder; old soundcraft.exe $OLDHASH"
echo "settings: ui.json and a FAKE control-token generated for this test (value not shown)"
log "1. wrong checksum"
run -InstallDir "$T/SML Studio" -ZipPath "$Z" -ExpectedSha256 0000 | head -4
echo "old exe unchanged: $(same "$T/SML Studio/soundcraft.exe" "$OLDHASH")"
log "2. app running from the folder"
cp /bin/sleep "$T/SML Studio/soundcraft"; ("$T/SML Studio/soundcraft" 30 &) ; sleep 1
run -InstallDir "$T/SML Studio" -ZipPath "$Z" | grep -i "running" | head -2
pkill -f "$T/SML Studio/soundcraft"; sleep 1; rm -f "$T/SML Studio/soundcraft"
echo "old exe unchanged: $(same "$T/SML Studio/soundcraft.exe" "$OLDHASH")"
log "3. -WhatIf"
run -InstallDir "$T/SML Studio" -ZipPath "$Z" -WhatIf | tail -2
echo "folders: $(ls "$T" | grep -v all-output | tr '\n' ' ')"
log "4. update"
run -InstallDir "$T/SML Studio" -ZipPath "$Z"
B=$(ls -d "$T/SML Studio.backup-"* | head -1)
echo "session kept in new folder: $(cat "$T/SML Studio/My Sessions/song.scraft")"
echo "settings original unchanged: $(cat "$T/appdata/SoundCraft/ui.json"); copy: $(cat "$B/settings-copy/ui.json")"
echo "token copy present: $([ -f "$B/settings-copy/control-token" ] && echo yes || echo NO); identical to the original: $(cmp -s "$B/settings-copy/control-token" "$T/appdata/SoundCraft/control-token" && echo yes || echo NO); mode: $(stat -c %a "$B/settings-copy/control-token")"
echo "new exe: $(sha256sum "$T/SML Studio/soundcraft.exe" | cut -d' ' -f1)"
log "5. rollback"
run -Rollback -InstallDir "$T/SML Studio" -BackupDir "$B"
echo "restored exe is the old one byte for byte: $(same "$T/SML Studio/soundcraft.exe" "$OLDHASH")"
echo "session still there: $(cat "$T/SML Studio/My Sessions/song.scraft")"
echo "folders: $(ls "$T" | grep -v all-output | tr '\n' ' ')"
log "6. the token never appears in any output or log"
nlogs=$(find "$T" -name 'update-*.log' | wc -l)
hits=$( { cat "$OUT"; find "$T" -name 'update-*.log' -exec cat {} +; } | grep -c "${FAKE:5}")
echo "searched: all script output and $nlogs update log(s)"
echo "occurrences of the token in them: $hits"
echo "control: the same search finds it in the backed-up token file: $(find "$T" -path '*settings-copy*' -name control-token -exec cat {} + | grep -c "${FAKE:5}")"
