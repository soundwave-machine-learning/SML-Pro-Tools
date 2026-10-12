from pathlib import Path
import hashlib,json,re
r=Path(__file__).resolve().parents[1];m=json.loads((r/'PACKAGE_MANIFEST.json').read_text())
actual={p.relative_to(r).as_posix() for p in r.rglob('*') if p.is_file() and p.name!='PACKAGE_MANIFEST.json'}
assert actual=={x['path'] for x in m['files']}
for x in m['files']:assert hashlib.sha256((r/x['path']).read_bytes()).hexdigest()==x['sha256'],x['path']
s=sorted((r/'docs/sprint_prompts').glob('SPRINT_*.md'));assert [p.name for p in s]==[f'SPRINT_{i:02}.md' for i in range(1,5)]
own=[]
for p in s:
 t=p.read_text();own+=re.findall(r'RC\d{2}',re.search(r'Requirement ownership: (.+)\.',t).group(1))
 for h in ['Objective','Prerequisites and starting state','Read first','Scope','Work','Required commands and evidence','Human/external gates','Documentation and review','Stop and checkpoint policy','Artifacts','Definition of Done','Automatic continuation gate','Commit and report']:assert '## '+h in t,(p.name,h)
assert sorted(own)==[f'RC{i:02}' for i in range(1,13)]
for p in r.rglob('*.md'):
 for dest in re.findall(r'\[[^\]]*\]\(([^)]+)\)',p.read_text()):
  if '://' not in dest and not dest.startswith('#'):assert (p.parent/dest.split('#')[0]).exists(),(p,dest)
print(f'PASS: {len(m["files"])} hashes; 4 substantive contracts; RC01–RC12 assigned once; local links valid. Document checks only.')
