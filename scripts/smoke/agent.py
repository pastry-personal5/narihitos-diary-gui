"""P1-M17..P1-M20 smoke: the built diary-server contains the embedded prompt assets and reports read-only
profile status through the real API with a fake `codex`, `claude` and `qwen`. Nothing here starts a run; synthetic only."""
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
binary = Path(os.environ.get('CARGO_TARGET_DIR', root / 'src-tauri/target')) / 'debug/diary-server'

# Embedding proof: Codex and Claude share their body text, so each phrase appears at least twice,
# and each Claude asset carries its own front matter.
data = binary.read_bytes()
for phrase in [b'celebrated, award-winning novelist', b'careful writing companion for a private diary']:
    assert data.count(phrase) >= 2, f'embedded prompt asset missing from the binary: {phrase!r}'
assert data.count(b'provider: claude\nmodel: claude-sonnet-4-6') >= 2, 'embedded Claude assets missing'
assert data.count(b'provider: qwen\nmodel: qwen3.6:35b-a3b') >= 2, 'embedded Qwen assets missing'

# The Apple helper is compiled by build.rs and embedded. Only the compiled Swift program carries its
# framework load path (the Rust code never names it). On a machine without swiftc or the SDK the
# embedded file is empty and the profile reports helper_missing.
helper_built = b'FoundationModels.framework' in data

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
    fake_qwen = base / 'qwen'
    fake_qwen.write_text('#!/bin/sh\ncase "$1" in\n  --version) echo "0.26.4";;\n  *) exit 64;;\nesac\n')
    fake_qwen.chmod(fake_qwen.stat().st_mode | stat.S_IXUSR)
    config = base / 'config.yaml'
    config.write_text(
        f'schema_version: 1\npaths:\n  entries_root: {workspace}\n  runtime_root: {runtime}\n'
        f'sources:\n  - {{alias: smoke-readable, type: narihito, root: {source}}}\n'
        f'agents:\n  profiles:\n'
        f'    - {{alias: main, type: codex, profile_home: {home}, executable: {fake}}}\n'
        f'    - {{alias: other, type: claude, profile_home: {home}, executable: {fake_claude}}}\n'
        f'    - {{alias: third, type: qwen, profile_home: {home}, executable: {fake_qwen}}}\n'
        f'    - {{alias: mac, type: apple-foundation-models}}\n'
        f'    - {{alias: gone, type: codex, profile_home: {home}, executable: {base}/no-such-codex}}\n'
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
        assert by['third']['status'] == 'unchecked' and by['third']['type'] == 'qwen', listing
        assert by['third']['capabilities']['chat'] == {'available': True, 'reason': None}, by['third']
        assert by['third']['capabilities']['generate'] == {'available': True, 'reason': None}, by['third']
        code, checked = call('/profiles/main/check', 'POST', session)
        assert code == 200 and checked['status'] == 'ready', (code, checked)
        assert call('/profiles', session=session)[1]['profiles'][0]['status'] == 'ready'
        code, checked_claude = call('/profiles/other/check', 'POST', session)
        assert code == 200 and checked_claude['status'] == 'ready', (code, checked_claude)
        code, checked_qwen = call('/profiles/third/check', 'POST', session)
        assert code == 200 and checked_qwen['status'] == 'ready', (code, checked_qwen)
        # Apple: availability only (no inference). Ready when Apple Intelligence is on here; otherwise one
        # closed reason; helper_missing exactly when the helper was not built.
        assert by['mac']['type'] == 'apple-foundation-models' and by['mac']['status'] == 'unchecked', listing
        assert by['mac']['capabilities']['chat'] == {'available': True, 'reason': None}, by['mac']
        code, checked_apple = call('/profiles/mac/check', 'POST', session)
        assert code == 200, (code, checked_apple)
        if checked_apple['status'] == 'ready':
            assert helper_built and checked_apple['reason'] is None, checked_apple
        else:
            assert checked_apple['reason'], checked_apple
            assert (checked_apple['reason'] == 'helper_missing') == (not helper_built), (helper_built, checked_apple)
        for p in (checked, checked_claude, checked_qwen):
            assert p['reason'] is None, p
        # A check that finds the command missing is a warning: it must reach the log, and `trace` must
        # show it as a typed agent.check line without any path.
        code, gone = call('/profiles/gone/check', 'POST', session)
        assert code == 200 and gone['status'] == 'missing' and gone['reason'] == 'executable_missing', (code, gone)
        traced = ''
        for _ in range(100):
            result = subprocess.run([str(binary), 'trace', '--config', str(config), '--select', 'recent:20'], capture_output=True, text=True)
            traced = result.stdout
            if '"code":"agent.check"' in traced and 'executable_missing' in traced:
                break
            time.sleep(0.05)
        assert '"code":"agent.check"' in traced and '"reason":"executable_missing"' in traced, traced
        assert str(base) not in traced and 'no-such-codex' not in traced, 'a path leaked into the trace'
        assert call('/profiles/nope/check', 'POST', session)[0] == 404
        code, settings = call('/settings', session=session)
        assert settings['agents_available'] is False, settings
        text = json.dumps(listing) + json.dumps(checked_apple) + json.dumps(checked) + json.dumps(checked_claude) + json.dumps(checked_qwen)
        assert str(base) not in text and 'codex-home' not in text, 'a path leaked into the profile response'
        assert not list((runtime).glob('agent-runs/*')), 'a probe left its run directory behind'
        if helper_built and checked_apple['reason'] != 'os_unsupported':
            native = list((runtime).glob('native/apple-fm-*'))
            assert len(native) == 1 and oct(native[0].stat().st_mode & 0o777) == '0o700', native
    finally:
        process.send_signal(signal.SIGINT)
        process.wait(timeout=10)
print('agent smoke: ok')
