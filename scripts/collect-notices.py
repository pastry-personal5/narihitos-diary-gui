"""Collect exact installed dependency notices without adding a license tool dependency."""
import hashlib
import json
from pathlib import Path
import re
import subprocess

root = Path(__file__).resolve().parent.parent
metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--manifest-path', str(root / 'src-tauri/Cargo.toml'), '--format-version', '1', '--offline']))
features = {node['id']: node['features'] for node in metadata['resolve']['nodes']}
output = root / 'assets/third-party'
licenses = output / 'licenses'
licenses.mkdir(parents=True, exist_ok=True)
records = []
missing = []

def notices(folder):
    files = []
    if folder.exists():
        for path in folder.iterdir():
            if re.search(r'license|licence|copying|copyright|notice', path.name, re.I):
                if path.is_file():
                    files.append(path)
                elif path.is_dir():
                    files.extend(p for p in path.rglob('*') if p.is_file())
    result = []
    for path in sorted(files):
        data = path.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        target = licenses / (digest + '.txt')
        target.write_bytes(data)
        result.append({'original_name': str(path.relative_to(folder)), 'file': 'licenses/' + target.name, 'sha256': digest})
    return result

for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
    if package['source'] is None:
        continue
    folder = Path(package['manifest_path']).parent
    texts = notices(folder)
    record = {'ecosystem': 'cargo', 'name': package['name'], 'version': package['version'], 'license': package['license'], 'source_archive': f"https://crates.io/api/v1/crates/{package['name']}/{package['version']}/download", 'features': features[package['id']], 'authors': package['authors'], 'notices': texts}
    records.append(record)
    if not texts:
        missing.append(record)

lock = json.loads((root / 'package-lock.json').read_text())
for path, package in sorted(lock['packages'].items()):
    if not path:
        continue
    folder = root / path
    name = package.get('name') or path.rsplit('node_modules/', 1)[-1]
    texts = notices(folder)
    record = {'ecosystem': 'npm', 'name': name, 'version': package['version'], 'license': package.get('license'), 'source_archive': package.get('resolved'), 'installed': folder.exists(), 'dev': package.get('dev', False), 'optional': package.get('optional', False), 'notices': texts}
    records.append(record)
    if folder.exists() and not texts:
        missing.append(record)

supplement_path = output / 'supplemental-notices.json'
if supplement_path.exists():
    supplemental = {(item['ecosystem'], item['name'], item['version']): item['notices'] for item in json.loads(supplement_path.read_text())}
    for record in records:
        record['notices'].extend(supplemental.get((record['ecosystem'], record['name'], record['version']), []))
missing = [record for record in records if not record['notices'] and (record['ecosystem'] == 'cargo' or record['installed'])]

def selected_licenses(expression):
    result = []
    for combined in expression.split(' AND '):
        alternatives = combined.strip('() ').replace('/', ' OR ').split(' OR ')
        choices = [value.strip() for value in alternatives]
        choice = 'MIT' if 'MIT' in choices else 'Apache-2.0' if 'Apache-2.0' in choices else choices[0]
        result.append(choice)
    return result
for record in records:
    record['selected_licenses'] = selected_licenses(record['license'])

(output / 'inventory.json').write_text(json.dumps(records, indent=2) + '\n')
used = {notice['file'] for record in records for notice in record['notices']}
# Remove only stale generated hashes from this task-owned output directory.
for file in licenses.glob('*.txt'):
    if 'licenses/' + file.name not in used:
        file.unlink()
print(f'{len(records)} resolved package records; {len(used)} distinct notice files')
for record in missing:
    print(f"Missing installed notice: {record['ecosystem']} {record['name']} {record['version']} ({record['license']})")
