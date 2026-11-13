#!/usr/bin/env python3
"""Isolated illustrated API proof. Live inference requires --live and DIARY_ILLUSTRATED_LIVE=1."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import uuid
from isolation import server_configuration

ROOT = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--live', action='store_true')
parser.add_argument('--gui-app', type=Path, help='Packaged scaffold-smoke app for the isolated native GUI leg')
args = parser.parse_args()
if args.live and args.gui_app:
    parser.error('--gui-app uses synthetic fixtures only')
if args.live and os.environ.get('DIARY_ILLUSTRATED_LIVE') != '1':
    parser.error('live mode requires DIARY_ILLUSTRATED_LIVE=1')
subprocess.run(['cargo', 'build', '--quiet', '--manifest-path', str(ROOT / 'src-tauri/Cargo.toml'), '-p', 'diary-server'], check=True)
target = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'src-tauri/target')).resolve()
binary = target / 'debug/diary-server'
if not args.live:
    subprocess.run(['cargo', 'build', '--quiet', '--manifest-path', str(ROOT / 'src-tauri/Cargo.toml'), '-p', 'diary-infrastructure', '--example', 'fake_cli'], check=True)
base = Path(tempfile.mkdtemp(prefix='diary-illustrated-', dir=ROOT / 'tmp' if args.live else None)).resolve()
os.chmod(base, 0o700)
process = None
try:
    workspace, runtime, home = [base / n for n in ['workspace', 'runtime', 'home']]
    for p in [workspace, runtime, home]:
        p.mkdir(mode=0o700)
    if args.live:
        config_text = (ROOT / 'config/global_config.yaml').read_text()
        profile = re.search(r'- alias: "xpp8"\n\s+type: codex\n\s+profile_home: "([^"]+)"', config_text)
        assert profile, 'xpp8 Codex profile not found'
        home = Path(profile[1])
        executable = Path(shutil.which('codex')).resolve()
        image_block = config_text[config_text.index('image_engines:'):]
        image_block = image_block.replace('scripts/run-krea2-diary"', 'scripts/run-krea2-diary.sh"')
        engine = 'krea2'
        alias = 'xpp8'
    else:
        executable = target / 'debug/examples/fake_cli'
        (home / 'scenario').write_text('illustrated')
        engine, alias = 'synthetic', 'synthetic'
        program = base / 'image-engine'
        program.write_text('#!' + shutil.which('python3') + '\n' + '''import os,sys,struct,zlib,time
from pathlib import Path
if sys.argv[1:]==['--check']:sys.exit(0)
control=Path(__file__).with_name('control')
mode=control.read_text() if control.exists() else ''
if mode=='fail':sys.exit(1)
if mode=='slow':time.sleep(60)
if mode=='gui':time.sleep(0.5)
assert os.environ['DIARY_IMAGE_PROMPT'].startswith('synthetic-trigger ')
def chunk(k,b):return struct.pack('>I',len(b))+k+b+struct.pack('>I',zlib.crc32(k+b))
png=b'\\x89PNG\\r\\n\\x1a\\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',1,1,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(b'\\x00\\xff\\x00\\x00'))+chunk(b'IEND',b'')
Path(os.environ['DIARY_IMAGE_OUT']).write_bytes(png)
''')
        program.chmod(0o700)
        image_block = f'image_engines:\n  synthetic:\n    label: Synthetic\n    program: {program}\n    trigger: synthetic-trigger\n    prompt_directives: [photo]\n'
    config = base / 'config.yaml'
    config.write_text(server_configuration() + f'schema_version: 1\npaths:\n  entries_root: {workspace}\n  runtime_root: {runtime}\nagents:\n  profiles:\n    - alias: {alias}\n      type: codex\n      profile_home: {home}\n      executable: {executable}\nillustrated:\n  max_photos: {1 if args.live else 4}\n' + image_block)
    config.chmod(0o600)
    log = (base / 'server.stderr').open('w')
    process = subprocess.Popen([str(binary), '--config', str(config)], stdout=subprocess.PIPE, stderr=log)
    record = None
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        assert process.poll() is None, 'isolated server exited; inspect retained stderr'
        for p in runtime.glob('*/discovery.json'):
            candidate = json.loads(p.read_text())
            if candidate['pid'] == process.pid:
                record = candidate
                break
        if record:
            break
        time.sleep(0.05)
    assert record, 'isolated server readiness timed out'
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    session = None
    def call(path, method='GET', body=None, authorized=True):
        headers = {'Authorization': 'Bearer ' + record['token'], 'Content-Type': 'application/json'}
        if authorized and session:
            headers['X-Diary-Session'] = session
        request = urllib.request.Request(record['origin'] + '/api/v1' + path, method=method, headers=headers, data=None if body is None else json.dumps(body).encode())
        try:
            response = opener.open(request, timeout=30)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            raw = response.read()
            return response.status, raw if response.headers.get('Content-Type') == 'image/png' else json.loads(raw) if raw else None
    session = call('/sessions', 'POST', {})[1]['session_id']
    date = '2026-11-13'
    assert call('/illustrated', authorized=False)[0] == 401
    saved = call('/authentic/' + date, 'PUT', {'operation_id': str(uuid.uuid4()), 'expected_revision': None, 'body': 'I had tea in the morning. Later I walked outside.\n', 'buffer_generation': 1})
    assert saved[0] == 200, saved
    authentic_path = workspace / f'assets/diary/authentic/2026/11/{date}.md'
    original = authentic_path.read_bytes()
    def start():
        payload = {'request_id': str(uuid.uuid4()), 'profile': alias, 'locale': 'en-US', 'engine': engine}
        status, result = call('/illustrated/' + date, 'POST', payload)
        assert status == 202, result
        assert call('/illustrated/' + date, 'POST', payload)[1]['job']['id'] == result['job']['id']
        return result['job']['id']
    def wait(job, expected):
        deadline = time.monotonic() + (2400 if args.live else 60)
        while time.monotonic() < deadline:
            status, result = call('/jobs/' + job)
            assert status == 200, result
            assert 'I had tea' not in json.dumps(result)
            if result['state'] in ['succeeded', 'failed', 'cancelled']:
                assert result['state'] == expected, result
                return result
            assert call('/sessions/' + session + '/heartbeat', 'POST')[0] == 204
            time.sleep(1)
        raise AssertionError('illustrated job timed out')
    started = time.monotonic()
    job = start()
    wait(job, 'succeeded')
    status, entry = call('/illustrated/' + date)
    assert status == 200 and entry['theme_count'] >= len(entry['photos']) >= 1, entry
    version = entry['version']
    for photo in entry['photos']:
        status, png = call(f'/illustrated/{date}/photos/{photo["file"]}?version={version}')
        assert status == 200 and png.startswith(b'\x89PNG\r\n\x1a\n')
    assert authentic_path.read_bytes() == original
    assert call('/authentic')[1]['items'][0]['date'] == date
    if not args.live:
        # Cancel while the real supervised fake agent is blocked, before any renderer starts.
        (home / 'scenario').write_text('silent-exec')
        (home / 'stdin').unlink(missing_ok=True)
        job = start()
        for _ in range(50):
            if (home / 'stdin').exists():
                break
            time.sleep(0.1)
        assert (home / 'stdin').exists(), 'agent never started'
        assert call('/jobs/' + job + '/cancel', 'POST')[0] == 200
        wait(job, 'cancelled')
        assert call('/illustrated/' + date)[1]['version'] == version
        (home / 'scenario').write_text('illustrated')
        (base / 'control').write_text('fail')
        wait(start(), 'failed')
        assert call('/illustrated/' + date)[1]['version'] == version
        (base / 'control').write_text('slow')
        job = start()
        for _ in range(30):
            if call('/jobs/' + job)[1].get('progress', {}).get('phase') == 'photo':
                break
            time.sleep(0.1)
        assert call('/jobs/' + job + '/cancel', 'POST')[0] == 200
        wait(job, 'cancelled')
        assert call('/illustrated/' + date)[1]['version'] == version
        (base / 'control').unlink()
        wait(start(), 'succeeded')
        new_version = call('/illustrated/' + date)[1]['version']
        assert new_version != version
        assert call(f'/illustrated/{date}/photos/01.png?version={version}')[0] == 409
        assert call('/illustrated/' + date, 'DELETE', {'version': new_version, 'confirmed': False})[0] == 409
        assert call('/illustrated/' + date, 'DELETE', {'version': new_version, 'confirmed': True})[0] == 200
        assert call('/illustrated/' + date)[0] == 404
        illustrated_root = workspace / 'assets/diary/illustrated/2026/11'
        assert not (illustrated_root / (date + '.md')).exists()
        assert not (illustrated_root / date).exists()
        assert authentic_path.read_bytes() == original
    gui_evidence = None
    if args.gui_app:
        assert call('/sessions/' + session, 'DELETE')[0] == 204
        session = None
        app = args.gui_app.resolve(strict=True)
        client = app / 'Contents/MacOS/diary-client'
        assert client.is_file(), 'packaged client missing'
        (base / 'control').write_text('gui')
        settings = base / 'gui-settings'
        settings.mkdir(mode=0o700)
        env = dict(os.environ, DIARY_ILLUSTRATED_GUI_SMOKE='1', DIARY_ILLUSTRATED_SMOKE_SETTINGS=str(settings))
        gui_log = base / 'gui.stderr'
        with gui_log.open('w') as output:
            gui = subprocess.Popen([str(client), '--connect', str(next(runtime.glob('*/discovery.json')))], env=env, stdout=output, stderr=output)
            try:
                deadline = time.monotonic() + 180
                while gui.poll() is None:
                    if time.monotonic() > deadline:
                        raise AssertionError('packaged Illustrated GUI timed out; inspect retained gui.stderr: ' + str(base))
                    lines = gui_log.read_text().splitlines()
                    if any('SMOKE illustrated_gui ' in line and 'failed' in line for line in lines):
                        raise AssertionError('packaged Illustrated GUI assertion failed; inspect retained gui.stderr: ' + str(base))
                    time.sleep(0.1)
            finally:
                if gui.poll() is None:
                    gui.terminate()
                    try:
                        gui.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        gui.kill()
                        gui.wait(timeout=5)
        lines = gui_log.read_text().splitlines()
        passed = [line for line in lines if 'SMOKE illustrated_gui ' in line and 'passed' in line]
        assert gui.returncode == 0 and passed, 'packaged Illustrated GUI did not pass; inspect retained gui.stderr: ' + str(base)
        assert not list((workspace / 'assets/diary/illustrated').rglob('*.md')), 'GUI deletion left Illustrated Markdown'
        assert not list((workspace / 'assets/diary/illustrated').rglob('*.png')), 'GUI deletion left generated photos'
        gui_evidence = {'client_sha256':hashlib.sha256(client.read_bytes()).hexdigest(), 'result':passed[-1]}
        print('PASS illustrated packaged GUI')
    evidence = {'profile': alias, 'engine': engine, 'job': job, 'photos': len(entry['photos']), 'themes': entry['theme_count'], 'authentic_unchanged': True, 'seconds': round(time.monotonic() - started, 2), 'server_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'gui': gui_evidence}
    (base / 'evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print('PASS illustrated live' if args.live else 'PASS illustrated synthetic')
    print('Evidence:', base)
finally:
    if process:
        process.terminate()
        try:
            process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
