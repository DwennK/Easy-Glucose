"""Exercise the shipped update helper against a disposable copy on the native CI runner."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
import zipfile

publish = Path(__file__).resolve().parents[1] / 'dist' / 'publish'
feed, = publish.glob('releases.*.json')
asset, = [item for item in json.loads(feed.read_bytes())['Assets'] if item['Type'] == 'Full']
package = publish / asset['FileName']
assert hashlib.sha256(package.read_bytes()).hexdigest() == asset['SHA256'].lower()
portable, = publish.glob('*.zip')
windows = sys.platform == 'win32'

with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    if windows:
        with zipfile.ZipFile(portable) as archive:
            archive.extractall(root)
        helper, = root.rglob('Update.exe')
    else:
        subprocess.run(['ditto', '-x', '-k', portable, root], check=True)
        app, = root.glob('*.app')
        helper = app / 'Contents/MacOS/UpdateMac'
        subprocess.run(['codesign', '--verify', '--deep', '--strict', app], check=True)

    # Use the real packaged installation, with an earlier installed version
    # recorded only in this temporary copy. Verify the replacement restores it.
    manifest, = [path for path in root.rglob('sq.version') if not path.is_symlink()]
    original = manifest.read_bytes()
    version_tag = f"<version>{asset['Version']}</version>".encode()
    assert version_tag in original
    manifest.write_bytes(original.replace(version_tag, b'<version>0.0.1</version>', 1))
    subprocess.run([helper, '--silent', 'apply', '--package', package, '--norestart'],
                   cwd=root.parent, check=True, timeout=120)
    assert ET.parse(manifest).findtext('.//{*}version') == asset['Version']
    if not windows:
        subprocess.run(['codesign', '--verify', '--deep', '--strict', app], check=True)
    print(f"Applied {package.name} and verified installed version {asset['Version']}.")

