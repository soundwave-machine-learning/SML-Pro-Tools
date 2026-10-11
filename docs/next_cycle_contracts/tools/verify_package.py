#!/usr/bin/env python3
"""Validate planning package integrity; never implies the DAW was tested."""
from pathlib import Path
import hashlib, json, re, sys
root=Path(__file__).resolve().parents[1]
manifest=json.loads((root/'PACKAGE_MANIFEST.json').read_text())
errors=[]
for f in manifest['files']:
    p=root/f['path']
    if not p.is_file(): errors.append('Missing '+f['path']); continue
    if hashlib.sha256(p.read_bytes()).hexdigest()!=f['sha256']:
        errors.append('Hash mismatch '+f['path'])
actual={p.relative_to(root).as_posix() for p in root.rglob('*') if p.is_file() and p.name!='PACKAGE_MANIFEST.json'}
if actual!={f['path'] for f in manifest['files']}: errors.append('Manifest inventory differs')
sprints=sorted((root/'docs/sprint_prompts').glob('SPRINT_*.md'))
if [p.name for p in sprints]!=[f'SPRINT_{i:02}.md' for i in range(1,5)]: errors.append('Expected exactly four sprint contracts')
all_owned=[]
for p in sprints:
    text=p.read_text()
    m=re.search(r'Requirement ownership: (.+)\.',text)
    owned=re.findall(r'R\d{2}',m.group(1)) if m else []
    all_owned+=owned
    if not owned: errors.append('No requirements '+p.name)
    for heading in ['Objective','Prerequisites and current evidence','Read first','Scope and interfaces','Work in dependency order','Required build and test execution','Measurements and experiments','Human and external work','Artifacts','Documentation updates','Definition of Done','Automatic continue gate','Stop conditions','Review','Commit, report and resume']:
        if '## '+heading not in text: errors.append('Missing section '+p.name+': '+heading)
if sorted(all_owned)!=[f'R{i:02}' for i in range(1,39)]: errors.append('Requirement ownership is missing or duplicated')
req=(root/'docs/REQUIREMENTS.md').read_text()
if re.findall(r'^\| (R\d{2}) \|',req,re.M)!=[f'R{i:02}' for i in range(1,39)]: errors.append('Requirement ledger incomplete')
for p in root.rglob('*.md'):
    for dest in re.findall(r'\[[^\]]*\]\(([^)]+)\)',p.read_text()):
        if '://' in dest or dest.startswith('#'): continue
        dest=dest.split('#')[0]
        if dest and not (p.parent/dest).exists(): errors.append('Broken local link '+p.name+': '+dest)
if errors:
    print('\n'.join(errors)); sys.exit(1)
print(f'PASS: {len(manifest["files"])} hashed files; exactly 4 sprint contracts; 38 requirements assigned once; relative links valid.')
print('Document validation only. No product build, native test or sonic acceptance is implied.')
