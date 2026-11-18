"""Bundle an already-built native binary; no network access and no signing secrets."""
import argparse
from pathlib import Path
import os
import plistlib
import shutil
import subprocess
import sys
import zipfile

ROOT = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser()
p.add_argument('--target', default='')
p.add_argument('--archive', action='store_true')
a = p.parse_args()
release = ROOT / 'target' / a.target / 'release'
dist = ROOT / 'dist'
dist.mkdir(exist_ok=True)
windows = 'windows' in a.target or sys.platform == 'win32'
if windows:
    bundle = dist / 'Easy-Glucose-Windows'
    bundle.mkdir(exist_ok=True)
    shutil.copy2(release / 'easy-glucose.exe', bundle / 'Easy Glucose.exe')
else:
    bundle = dist / 'Easy Glucose.app'
    contents = bundle / 'Contents'
    (contents / 'MacOS').mkdir(parents=True, exist_ok=True)
    (contents / 'Resources').mkdir(exist_ok=True)
    shutil.copy2(release / 'easy-glucose', contents / 'MacOS' / 'easy-glucose')
    os.chmod(contents / 'MacOS' / 'easy-glucose', 0o755)
    shutil.copy2(ROOT / 'assets/icon.icns', contents / 'Resources' / 'AppIcon.icns')
    with (contents / 'Info.plist').open('wb') as f:
        plistlib.dump({'CFBundleName':'Easy Glucose','CFBundleDisplayName':'Easy Glucose',
            'CFBundleIdentifier':'ch.peaks.easy-glucose','CFBundleVersion':'1',
            'CFBundleShortVersionString':'0.1.0','CFBundlePackageType':'APPL',
            'CFBundleExecutable':'easy-glucose','CFBundleIconFile':'AppIcon',
            'NSHighResolutionCapable':True,'LSMinimumSystemVersion':'12.0',
            'NSHumanReadableCopyright':'Copyright 2026 Easy Glucose contributors'}, f)
for name in ['README.md','LICENSE','THIRD_PARTY.md']:
    destination = bundle if windows else bundle / 'Contents' / 'Resources'
    shutil.copy2(ROOT / name, destination / name)
shutil.copytree(ROOT/'docs', (bundle if windows else bundle/'Contents'/'Resources')/'docs', dirs_exist_ok=True)
# Include original license/notice files for dependencies in distribution archives.
licenses = ROOT / 'dist' / 'dependency-licenses'
if licenses.exists():
    shutil.copytree(licenses, (bundle if windows else bundle/'Contents'/'Resources')/'licenses', dirs_exist_ok=True)
if not windows:
    subprocess.run(['codesign','--force','--deep','--sign','-',str(bundle)],check=True)
if a.archive:
    target = a.target or ('macos-arm64' if os.uname().machine=='arm64' else 'native')
    archive = dist / f'Easy-Glucose-{target}.zip'
    if windows:
        with zipfile.ZipFile(archive,'w',zipfile.ZIP_DEFLATED) as z:
            for path in bundle.rglob('*'):
                if path.is_file(): z.write(path,path.relative_to(dist))
    else:
        subprocess.run(['ditto','-c','-k','--sequesterRsrc','--keepParent',str(bundle),str(archive)],check=True)
    print(archive)
print(bundle)
