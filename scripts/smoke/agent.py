"""P1-M17/P1-M18 smoke: the built diary-server contains the embedded prompt assets and reports read-only
profile status through the real API with a fake `codex` and a fake `claude`. Nothing here starts a run; synthetic only."""
import json
import os
from pathlib import Path
import signal
import stat
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

root = Path(__file__).resolve().parents[2]
subprocess.run(['cargo', 'build', '--quiet', '--manifest-path', str(root / 'src-tauri/Cargo.toml'), '-p', 'diary-server'], check=True)
binary = root / 'src-tauri/target/debug/diary-server'

# Embedding proof: Codex and Claude share their body text, so each phrase appears at least twice,
# and each Claude asset carries its own front matter.
data = binary.read_bytes()
for phrase in [b'celebrated, award-winning novelist', b'careful writing companion for a private diary']:
    assert data.count(phrase) >= 2, f'embedded prompt asset missing from the binary: {phrase!r}'
assert data.count(b'provider: claude\nmodel: claude-sonnet-4-6') >= 2, 'embedded Claude assets missing'

with tempfile.TemporaryDirectory(prefix='diary-agent-smoke-') as temporary:
    base = Path(temporary).resolve()
    workspace, runtime, source, home = base / 'workspace', base / 'runtime', base / 'source', base / 'codex-home'
    for folder in [base, workspace, runtime, source, home]:
        folder.mkdir(exist_ok=True, mode=0o700)
        folder.chmod(0o700)
    fake = base / 'codex'
    fake.write_text('#!/bin/sh\ncase "$1" in\n  --version) echo "codex-cli 0.167.0";;\n  login) echo "Logged in using ChatGPT";;\n  *) exit 64;;\nesac\n')
    fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
    fake_claude = base / 'claude'
    fake_claude.write_text('#!/bin/sh\ncase "$1" in\n  --version) echo "2.1.319 (Claude Code)";;\n  auth) echo "{\\"loggedIn\\": true}";;\n  *) exit 64;;\nesac\n')
    fake_claude.chmod(fake_claude.stat().st_mode | stat.S_IXUSR)
    config = base / 'config.yaml'
    config.write_text(
        f'schema_version: 1\npaths:\n  entries_root: {workspace}\n  runtime_root: {runtime}\n'
        f'sources:\n  - {{alias: smoke-readable, type: narihito, root: {source}}}\n'
        f'agents:\n  profiles:\n'
        f'    - {{alias: main, type: codex, profile_home: {home}, executable: {fake}}}\n'
        f'    - {{alias: other, type: claude, profile_home: {home}, executable: {fake_claude}}}\n'
    )
    config.chmod(0o600)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    process = subprocess.Popen([str(binary), '--config', str(config)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        record = None
        for _ in range(300):
            for found in runtime.glob('*/discovery.json'):
                candidate = json.loads(found.read_text())
                if candidate['pid'] == process.pid:
                    record = candidate
            if record:
                break
            assert process.poll() is None, 'server exited before readiness'
            time.sleep(0.02)
        assert record, 'server readiness timeout'
        auth = {'Authorization': 'Bearer ' + record['token']}

        def call(path, method='GET', session=None, headers=None):
            h = dict(auth, **(headers or {}))
            if session:
                h['X-Diary-Session'] = session
            request = urllib.request.Request(record['origin'] + '/api/v1' + path, data=b'{}' if method == 'POST' else None, headers=dict(h, **{'Content-Type': 'application/json'}), method=method)
            try:
                with opener.open(request, timeout=10) as response:
                    return response.status, json.loads(response.read() or b'null')
            except urllib.error.HTTPError as error:
                return error.code, json.loads(error.read() or b'null')

        code, opened = call('/sessions', 'POST')
        session = opened['session_id']
        assert call('/profiles')[0] == 401  # no session
        code, listing = call('/profiles', session=session)
        by = {p['alias']: p for p in listing['profiles']}
        assert code == 200 and by['main']['status'] == 'unchecked' and by['main']['type'] == 'codex', listing
        assert by['main']['capabilities']['chat'] == {'available': True, 'reason': None}, by['main']
        assert by['other']['status'] == 'unchecked' and by['other']['type'] == 'claude', listing
        assert by['other']['capabilities']['chat'] == {'available': True, 'reason': None}, by['other']
        assert by['other']['capabilities']['generate'] == {'available': True, 'reason': None}, by['other']
        code, checked = call('/profiles/main/check', 'POST', session)
        assert code == 200 and checked['status'] == 'ready', (code, checked)
        assert call('/profiles', session=session)[1]['profiles'][0]['status'] == 'ready'
        code, checked_claude = call('/profiles/other/check', 'POST', session)
        assert code == 200 and checked_claude['status'] == 'ready', (code, checked_claude)
        assert call('/profiles/nope/check', 'POST', session)[0] == 404
        code, settings = call('/settings', session=session)
        assert settings['agents_available'] is False, settings
        text = json.dumps(listing) + json.dumps(checked) + json.dumps(checked_claude)
        assert str(base) not in text and 'codex-home' not in text, 'a path leaked into the profile response'
        assert not list((runtime).glob('agent-runs/*')), 'a probe left its run directory behind'
    finally:
        process.send_signal(signal.SIGINT)
        process.wait(timeout=10)
print('agent smoke: ok')
