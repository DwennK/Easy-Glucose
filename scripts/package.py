"""Bundle an already-built native binary; no network access and no signing secrets."""
import argparse
import json
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
p.add_argument('--updates', action='store_true', help='Create Velopack distribution files')
a = p.parse_args()
metadata = json.loads(subprocess.check_output(
    ['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1'], cwd=ROOT
))
version = next(p['version'] for p in metadata['packages'] if p['name'] == 'easy-glucose')
if os.environ.get('GITHUB_REF_TYPE') == 'tag' and os.environ.get('GITHUB_REF_NAME') != f'v{version}':
    raise SystemExit(f'Release tag must match Cargo.toml: expected v{version}')

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
            'CFBundleIdentifier':'ch.peaks.easy-glucose','CFBundleVersion':version,
            'CFBundleShortVersionString':version,'CFBundlePackageType':'APPL',
            'CFBundleExecutable':'easy-glucose','CFBundleIconFile':'AppIcon',
            'NSHighResolutionCapable':True,'LSMinimumSystemVersion':'12.0',
            'NSHumanReadableCopyright':'Copyright 2026 Easy Glucose contributors'}, f)
for name in ['README.md','LICENSE','THIRD_PARTY.md']:
    destination = bundle if windows else bundle / 'Contents' / 'Resources'
    shutil.copy2(ROOT / name, destination / name)
shutil.copy2(ROOT/'assets'/'sounds'/'g7'/'README.md',
             (bundle if windows else bundle/'Contents'/'Resources')/'Dexcom-G7-audio.md')
shutil.copytree(ROOT/'docs', (bundle if windows else bundle/'Contents'/'Resources')/'docs', dirs_exist_ok=True)
# Include original license/notice files for dependencies in distribution archives.
licenses = ROOT / 'dist' / 'dependency-licenses'
if licenses.exists():
    shutil.copytree(licenses, (bundle if windows else bundle/'Contents'/'Resources')/'licenses', dirs_exist_ok=True)
if not windows:
    subprocess.run(['codesign','--force','--deep','--sign','-',str(bundle)],check=True)
if a.updates:
    channels = {
        'x86_64-pc-windows-msvc': ('win-x64', 'Easy Glucose.exe'),
        'aarch64-apple-darwin': ('osx-arm64', 'easy-glucose'),
        'x86_64-apple-darwin': ('osx-x64', 'easy-glucose'),
    }
    channel, executable = channels[a.target]
    output = dist / 'velopack' / channel
    publish = dist / 'publish'
    publish.mkdir(exist_ok=True)
    command = ['vpk', 'pack', '--packId', 'EasyGlucose', '--packTitle', 'Easy Glucose',
               '--packVersion', version, '--packDir', str(bundle), '--mainExe', executable,
               '--channel', channel, '--runtime', channel, '--outputDir', str(output),
               '--icon', str(ROOT / 'assets' / ('icon.ico' if windows else 'icon.icns'))]
    if not windows:
        command.extend(['--signAppIdentity=-', '--noInst'])
    subprocess.run(command, check=True)
    base = f'EasyGlucose-{channel}'
    feed = output / f'releases.{channel}.json'
    assets = json.loads(feed.read_bytes())['Assets']
    package, = [item for item in assets if item['Version'] == version and item['Type'] == 'Full']
    shutil.copy2(output / package['FileName'], publish)
    shutil.copy2(feed, publish)
    shutil.copy2(output / f'{base}-Portable.zip', publish / f'Easy-Glucose-{a.target}.zip')
    if windows:
        shutil.copy2(output / f'{base}-Setup.exe', publish / 'Easy-Glucose-Setup.exe')
elif a.archive:
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
