"""Collect resolved dependency notices, without adding a build-time binary dependency."""
import json
import subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
metadata=json.loads(subprocess.check_output(['cargo','metadata','--format-version','1','--locked'],cwd=ROOT))
lines=['# Third-party dependencies','', 'Resolved packages in Cargo.lock (including build and platform-specific dependencies).', '', '| Package | Version | Declared license |','| --- | --- | --- |']
output=ROOT/'dist/dependency-licenses'
for package in sorted(metadata['packages'],key=lambda p:(p['name'],p['version'])):
    if package['name']=='easy-glucose': continue
    lines.append(f"| {package['name']} | {package['version']} | {package.get('license') or 'See upstream notice'} |")
    base=Path(package['manifest_path']).parent
    dest=output/f"{package['name']}-{package['version']}"
    files=[p for p in base.rglob('*') if p.is_file() and (p.name.lower().startswith(('license','licence','copying','notice','copyright'))) and p.stat().st_size<1000000]
    for file in files:
        path=dest/file.relative_to(base)
        path.parent.mkdir(parents=True,exist_ok=True)
        path.write_bytes(file.read_bytes())
(ROOT/'THIRD_PARTY.md').write_text('\n'.join(lines)+'\n')
