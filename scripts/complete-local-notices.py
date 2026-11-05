"""Supplement package notice locations and explicitly shared upstream license texts."""
import hashlib
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parent.parent
base = root / 'assets/third-party'
metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--manifest-path', str(root / 'src-tauri/Cargo.toml'), '--format-version', '1', '--offline']))
packages = {(p['name'], p['version']): p for p in metadata['packages']}
supplements = json.loads((base / 'supplemental-notices.json').read_text())

def notice(data, name, provenance):
    digest = hashlib.sha256(data).hexdigest()
    (base / 'licenses' / (digest + '.txt')).write_bytes(data)
    return {'original_name': name, 'file': 'licenses/' + digest + '.txt', 'sha256': digest, 'provided_by': provenance}

def put(ecosystem, name, version, notices):
    global supplements
    supplements = [p for p in supplements if (p['ecosystem'], p['name'], p['version']) != (ecosystem, name, version)]
    supplements.append({'ecosystem':ecosystem,'name':name,'version':version,'notices':notices})

def cargo_file(name, version, filename):
    folder = Path(packages[(name, version)]['manifest_path']).parent
    return (folder / filename).read_bytes()

for version in ['5.3.0', '6.0.0']:
    put('cargo', 'r-efi', version, [notice(cargo_file('r-efi',version,'AUTHORS'),'AUTHORS',f'r-efi {version} source archive')])
parent = packages[('libappindicator','0.9.0')]
folder = Path(parent['manifest_path']).parent
texts = [notice(path.read_bytes(),path.name,'libappindicator 0.9.0: identical recorded repository commit eafd1e3682a1247f595410266091e9684021cb6f') for path in folder.iterdir() if path.name.startswith('LICENSE')]
put('cargo','libappindicator-sys','0.9.0',texts)
for name in ['winapi-i686-pc-windows-gnu','winapi-x86_64-pc-windows-gnu']:
    declaration = cargo_file(name,'0.4.0','Cargo.toml.orig')
    put('cargo',name,'0.4.0',[notice(declaration,'Cargo.toml.orig: published author and MIT/Apache-2.0 declaration',f'{name} 0.4.0 source archive'),notice((root/'LICENSE').read_bytes(),'Apache-2.0 standard license text','Apache-2.0 text, as declared by this package; no LGPL selection')])
selector_header = cargo_file('selectors','0.38.0','lib.rs').split(b'*/',1)[0] + b'*/\n'
put('cargo','selectors','0.38.0',[notice(selector_header,'lib.rs: original MPL notice','selectors 0.38.0 source archive'),notice(cargo_file('cssparser','0.37.0','LICENSE'),'MPL-2.0 standard license text','cssparser 0.37.0: same unmodified MPL-2.0 standard license text')])
for binary, wrapper, version in [('@rolldown/binding-darwin-arm64','rolldown','1.2.16'),('@tauri-apps/cli-darwin-arm64','@tauri-apps/cli','2.12.4')]:
    folder=root/'node_modules'/wrapper
    texts=[notice(path.read_bytes(),path.name,f'{wrapper} {version}: same-version official wrapper for the platform binary') for path in folder.iterdir() if path.is_file() and 'LICENSE' in path.name]
    put('npm',binary,version,texts)
readme=(root/'node_modules/imurmurhash/README.md').read_bytes()
mit=readme[readme.index(b'License (MIT)'):]
put('npm','imurmurhash','0.1.4',[notice(mit,'README.md: complete MIT section','imurmurhash 0.1.4 npm archive')])
source=(root/'node_modules/esrecurse/esrecurse.js').read_bytes()
put('npm','esrecurse','4.3.0',[notice(source[:source.index(b'*/')+2]+b'\n','esrecurse.js: original BSD notice','esrecurse 4.3.0 npm archive')])
readme=(root/'node_modules/natural-compare/README.md').read_bytes()
section=readme[readme.rindex(b'Licence'):]
terms=mit[mit.index(b'Permission is hereby'):]
put('npm','natural-compare','1.4.0',[notice(section,'README.md: original copyright/MIT notice','natural-compare 1.4.0 npm archive'),notice(terms,'MIT standard permission/warranty text','imurmurhash 0.1.4: same MIT permission/warranty text; copyright remains the original package notice')])
put('npm','@humanfs/types','0.15.0',[notice((root/'node_modules/@humanfs/types/README.md').read_bytes(),'README.md: original author and Apache-2.0 declaration','@humanfs/types 0.15.0 npm archive'),notice((root/'LICENSE').read_bytes(),'Apache-2.0 standard license text','Apache-2.0 standard text, as declared by @humanfs/types 0.15.0')])
(base/'supplemental-notices.json').write_text(json.dumps(supplements,indent=2)+'\n')
print('Local source/README and official platform-wrapper supplements collected')
