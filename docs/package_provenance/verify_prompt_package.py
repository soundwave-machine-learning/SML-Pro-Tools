#!/usr/bin/env python3
"""Verify this original prompt bundle before copying/editing it; no product tests."""
from pathlib import Path
import hashlib,json,re,sys,zipfile
root=Path(__file__).resolve().parents[1]
manifest=json.loads((root/'PACKAGE_MANIFEST.json').read_text())
errors=[]
if manifest.get('revision')!='1.6': errors.append('Expected revision 1.6')
actual={str(p.relative_to(root)) for p in root.rglob('*') if p.is_file() and p.name!='PACKAGE_MANIFEST.json' and '__pycache__' not in p.parts}
if actual!=set(manifest['files']): errors.append('File inventory mismatch')
for name,meta in manifest['files'].items():
 p=root/name
 if not p.is_file():errors.append('Missing '+name);continue
 data=p.read_bytes()
 if len(data)!=meta['bytes'] or hashlib.sha256(data).hexdigest()!=meta['sha256']: errors.append('Hash/size mismatch '+name)
contracts=sorted((root/'docs/sprint_prompts').glob('*.md'))
if [p.name for p in contracts]!=[f'SPRINT_0{i}.md' for i in range(1,5)]:errors.append('Sprint count/name mismatch')
for p in contracts:
 s=p.read_text()
 for part in ['Objective','Read set','Concrete work','Validation','Execution, report']:
  if part not in s:errors.append(p.name+' lacks '+part)
 if 'Revision 1.6' not in s:errors.append(p.name+' revision mismatch')
master=(root/'docs/AUTONOMOUS_BUILD_PROMPT.md').read_text()
for i in range(1,5):
 if f'docs/sprint_prompts/SPRINT_0{i}.md' not in master:errors.append('Master lacks sprint '+str(i))
matrix=(root/'docs/REQUIREMENTS_MATRIX.md').read_text()
for i in range(1,27):
 if f'| R{i:02} |' not in matrix:errors.append('Matrix missing requirement '+str(i))
# Only planning paths; execution evidence/reports/manuals intentionally do not yet exist.
for p in root.rglob('*.md'):
 s=p.read_text()
 for ref in re.findall(r'(?<![A-Za-z0-9_/])(?:docs/)?[A-Z][A-Z0-9_]*\.md',s):
  if ref.endswith(('USER_GUIDE.md','TARGET_MACHINE_GAUNTLET.md','AUTONOMOUS_BUILD_FINAL_REPORT.md','AGENTS.md','README.md','ROADMAP.md')):continue
  name=ref.split('/')[-1]
  if not any(root.rglob(name)):errors.append(str(p.relative_to(root))+' missing referenced file '+ref)
 if p.name!='CHANGELOG.md' and re.search(r'^## (?:Mandatory revision|Current package revision|Revision 1\.[2345])',s,re.M):errors.append('Stale amendment heading '+str(p.relative_to(root)))
if errors:
 print('FAIL\n'+'\n'.join(errors));sys.exit(1)
print(f'PASS: revision 1.6, {len(actual)+1} files, four substantive contracts, R01–R26 coverage, hashes and reference checks. Product tests NOT run.')
