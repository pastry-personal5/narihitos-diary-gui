"""Fetch missing license assets only from a crate's recorded upstream Git commit."""
import concurrent.futures
import hashlib
import json
from pathlib import Path
import re
import subprocess
import urllib.request

root = Path(__file__).resolve().parent.parent
metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--manifest-path', str(root / 'src-tauri/Cargo.toml'), '--format-version', '1', '--offline']))
base = root / 'assets/third-party'
records = json.loads((base / 'inventory.json').read_text())
missing = {(r['name'], r['version']) for r in records if r['ecosystem'] == 'cargo' and not r['notices']}
groups = {}
for p in metadata['packages']:
    if (p['name'], p['version']) not in missing:
        continue
    folder = Path(p['manifest_path']).parent
    vcs = json.loads((folder / '.cargo_vcs_info.json').read_text()) if (folder / '.cargo_vcs_info.json').exists() else {}
    repository = (p.get('repository') or '').rstrip('/').removesuffix('.git')
    commit = vcs.get('git', {}).get('sha1')
    if not repository.startswith('https://github.com/') or not commit:
        print('Needs manual provenance:', p['name'], p['version'])
        continue
    repo = repository.removeprefix('https://github.com/')
    groups.setdefault((repo, commit), []).append((p['name'], p['version'], vcs.get('path_in_vcs', '')))

def fetch(url):
    return urllib.request.urlopen(urllib.request.Request(url, headers={'User-Agent':'Diary dependency notice collector'}), timeout=20).read()

def collect(group):
    (repo, commit), packages = group
    results = []
    try:
        tree = json.loads(fetch(f'https://api.github.com/repos/{repo}/git/trees/{commit}?recursive=1'))
        if tree.get('truncated'):
            return [], f'{repo} {commit}: truncated tree'
        for name, version, crate_path in packages:
            found = []
            ancestors = {str(parent) if str(parent) != '.' else '' for parent in Path(crate_path).parents} | {crate_path, ''}
            for entry in tree.get('tree', []):
                path = Path(entry['path'])
                parent = '' if str(path.parent) == '.' else str(path.parent)
                if entry['type'] != 'blob' or parent not in ancestors:
                    continue
                if not re.match(r'^(licen[cs]e|copying|notice|copyright)([._-].*|$)', path.name, re.I):
                    continue
                url = f'https://raw.githubusercontent.com/{repo}/{commit}/{entry["path"]}'
                content = fetch(url)
                digest = hashlib.sha256(content).hexdigest()
                (base / 'licenses' / (digest + '.txt')).write_bytes(content)
                found.append({'original_name':entry['path'],'file':'licenses/' + digest + '.txt','sha256':digest,'source_url':url})
            results.append({'ecosystem':'cargo','name':name,'version':version,'notices':found})
        return results, None
    except Exception as error:
        return [], f'{repo} {commit}: {type(error).__name__}'

supplements = json.loads((base / 'supplemental-notices.json').read_text()) if (base / 'supplemental-notices.json').exists() else []
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
    for result, error in pool.map(collect, groups.items()):
        keys = {(item['ecosystem'], item['name'], item['version']) for item in result}
        supplements = [item for item in supplements if (item['ecosystem'], item['name'], item['version']) not in keys] + result
        if error:
            print(error)
(base / 'supplemental-notices.json').write_text(json.dumps(supplements, indent=2) + '\n')
for item in supplements:
    print(item['name'], item['version'], len(item['notices']), 'upstream notices')
