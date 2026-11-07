"""P1-M17..P1-M23: embedded assets, profile status and generation on the built server.
Only fake CLIs run inference-shaped fixtures; Apple is checked without inference."""
import json
import os
from pathlib import Path
import signal
import shlex
import uuid
import stat
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

root = Path(__file__).resolve().parents[2]
subprocess.run(['cargo', 'build', '--quiet', '--manifest-path', str(root / 'src-tauri/Cargo.toml'), '-p', 'diary-server'], check=True)
binary = Path(os.environ.get('CARGO_TARGET_DIR', root / 'src-tauri/target')) / 'debug/diary-server'
subprocess.run(['cargo', 'build', '--quiet', '--manifest-path', str(root / 'src-tauri/Cargo.toml'), '-p', 'diary-infrastructure', '--example', 'fake_cli'], check=True)
fake_binary = binary.parent / 'examples/fake_cli'
SOURCE_SECRET = 'SYNTHETIC_GENERATION_SOURCE_SENTINEL'
DATE = '2026-11-06'

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
    claude_home, qwen_home = base / 'claude-home', base / 'qwen-home'
    for folder in [base, workspace, runtime, source, home, claude_home, qwen_home]:
        folder.mkdir(exist_ok=True, mode=0o700)
        folder.chmod(0o700)
    # Counts actual fake child runs and optionally fails only the second candidate.
    wrapper = '#!/bin/sh\n' + r'''case "$1" in
--version|login|auth) ;;
*)
  H="${CODEX_HOME:-${CLAUDE_CONFIG_DIR:-$QWEN_HOME}}"
  printf . >> "$H/runs"
  if test -f "$H/partial"; then
    N=$(wc -c < "$H/runs" | tr -d ' ')
    if test "$N" = 2; then echo malformed > "$H/scenario"; else : > "$H/scenario"; fi
  fi;;
esac
''' + 'exec ' + shlex.quote(str(fake_binary)) + ' "$@"\n'
    fake, fake_claude, fake_qwen = base / 'codex', base / 'claude', base / 'qwen'
    for executable in [fake, fake_claude, fake_qwen]:
        executable.write_text(wrapper)
        executable.chmod(executable.stat().st_mode | stat.S_IXUSR)
    file = source / '2026/11/06/2026-11-06_00-00-00-123.md'
    file.parent.mkdir(parents=True)
    file.write_text('---\nid: "2026-11-06_00-00-00-123"\ncreated_at: "2026-11-06T00:00:00.123Z"\ntags: []\n---\n' + SOURCE_SECRET + '. I had tea.\n')
    config = base / 'config.yaml'
    config.write_text(
        f'schema_version: 1\npaths:\n  entries_root: {workspace}\n  runtime_root: {runtime}\n'
        f'sources:\n  - {{alias: smoke-readable, type: ayumi, root: {source}}}\n'
        f'agents:\n  profiles:\n'
        f'    - {{alias: main, type: codex, profile_home: {home}, executable: {fake}}}\n'
        f'    - {{alias: other, type: claude, profile_home: {claude_home}, executable: {fake_claude}}}\n'
        f'    - {{alias: third, type: qwen, profile_home: {qwen_home}, executable: {fake_qwen}}}\n'
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

        def call(path, method='GET', session=None, headers=None, body=None):
            h = dict(auth, **(headers or {}))
            if session:
                h['X-Diary-Session'] = session
            request = urllib.request.Request(record['origin'] + '/api/v1' + path, data=(json.dumps(body).encode() if body is not None else b'{}') if method in ('POST', 'DELETE') else None, headers=dict(h, **{'Content-Type': 'application/json'}), method=method)
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
        assert settings['agents_available'] is True, settings  # a runner is wired and a profile is configured
        # Chat routes (P1-M21): refusals and open/close only; no turn is sent, so no model runs.
        code, refused = call('/chats', 'POST', session, body={'profile': 'gone', 'locale': 'en-US'})
        assert code == 409 and refused['error']['code'] == 'profile_not_ready', (code, refused)
        assert call('/chats', 'POST', session, body={'profile': 'nope', 'locale': 'en-US'})[0] == 404
        assert call('/chats', 'POST', session, body={'profile': 'main', 'locale': 'not a locale'})[0] == 400
        code, opened_chat = call('/chats', 'POST', session, body={'profile': 'main', 'locale': 'en-US'})
        assert code == 201 and opened_chat['locale'] == 'en-US', (code, opened_chat)
        code, chat_view = call('/chats/' + opened_chat['chat_id'], session=session)
        assert code == 200 and chat_view['transcript'] == [] and chat_view['turn'] is None, chat_view
        assert call('/chats/' + opened_chat['chat_id'], 'DELETE', session)[0] == 200
        assert call('/chats/' + opened_chat['chat_id'], session=session)[0] == 404
        # Generation refusals, immutable import capture, partial outcomes, stale baseline,
        # individual reads, retry/discard and cancellation over real HTTP/process supervision.
        code, imported = call('/imports', 'POST', session, body={'date': DATE, 'request_id': 'generation-import'})
        assert code == 202, (code, imported)
        import_id = imported['import_id']
        def wait_job(job_id, limit=20):
            end = time.monotonic() + limit
            while time.monotonic() < end:
                code, job = call('/jobs/' + job_id, session=session)
                assert code == 200, (code, job)
                if job['state'] in ('succeeded', 'failed', 'cancelled'):
                    return job
                call('/sessions/' + session + '/heartbeat', 'POST', session)
                time.sleep(0.02)
            raise AssertionError('generation/import timeout')
        assert wait_job(import_id)['state'] == 'succeeded'
        code, page = call('/sources?import_id=' + import_id, session=session)
        assert code == 200, (code, page)
        selected = [{'import_id': import_id, 'source_id': row['source']['id']} for row in page['items'] if row['kind'] == 'included']
        assert len(selected) == 1, page
        def generation_input(request_id, profile='main', count=1):
            return {'request_id': request_id, 'date': DATE, 'profile': profile, 'locale': 'en-US', 'count': count, 'selections': selected}
        empty = generation_input('empty')
        empty['selections'] = []
        code, refused = call('/generations', 'POST', session, body=empty)
        assert code == 422 and refused['error']['code'] == 'empty_selection', (code, refused)
        assert call('/generations', 'POST', session, body=generation_input('off', 'gone'))[0] == 409
        for alias, profile_home in [('main', home), ('other', claude_home), ('third', qwen_home)]:
            payload = generation_input('generation-' + alias, alias)
            code, accepted = call('/generations', 'POST', session, body=payload)
            assert code == 202, (code, accepted)
            job_id = accepted['job']['id']
            assert wait_job(job_id)['state'] == 'succeeded'
            # Simulate a frontend that lost admission/result state: discover by request identity.
            code, jobs = call('/jobs', session=session)
            assert code == 200, (code, jobs)
            discovered = [j for j in jobs['jobs'] if j.get('progress', {}).get('request_id') == payload['request_id']]
            assert len(discovered) == 1 and discovered[0]['id'] == job_id, discovered
            projection = discovered[0]['progress']
            assert projection['session_id'] == session and projection['profile_id'] == alias
            assert projection['origin_date'] == DATE and projection['candidate_ids'] == [job_id + '-1']
            assert isinstance(projection['silence_ms'], int)
            # A fresh event consumer must catch up before claiming a usable baseline.
            stream_request = urllib.request.Request(record['origin'] + '/api/v1/events', headers=dict(auth, **{
                'X-Diary-Session': session, 'X-Diary-Reconcile': '1',
                'Last-Event-ID': str(jobs['event_cursor'])}))
            with opener.open(stream_request, timeout=5) as stream:
                assert stream.headers.get('x-diary-reconcile') == '1'
                frames = []
                while True:
                    line = stream.readline().decode()
                    assert line, 'stream ended before replay completion'
                    frames.append(line)
                    if line.startswith('event: replay_complete'):
                        break
                encoded = ''.join(frames)
                assert 'resync_required' not in encoded, encoded
                assert SOURCE_SECRET not in encoded and 'Tea was nice' not in encoded
            assert SOURCE_SECRET not in json.dumps(jobs) and 'Tea was nice' not in json.dumps(jobs)
            assert call('/sessions/' + session + '/heartbeat', 'POST', session)[0] == 204
            code, manifest = call('/generations/' + job_id, session=session)
            assert code == 200 and manifest['progress']['ready_count'] == 1, (code, manifest)
            assert SOURCE_SECRET not in json.dumps(manifest) and 'Tea was nice' not in json.dumps(manifest)
            assert 'provisional' not in manifest and manifest['stale_baseline'] is False
            candidate_id = manifest['candidates'][0]['id']
            code, candidate = call('/candidates/' + candidate_id, session=session)
            assert code == 200 and candidate['draft_markdown'] == 'Tea was nice.', (code, candidate)
            assert call('/generations', 'POST', session, body=payload)[1]['job']['id'] == job_id
            assert (profile_home / 'runs').read_text() == '.', 'a retry ran inference again'
            assert call('/generations/' + job_id, 'DELETE', session)[0] == 200
            retained = call('/jobs/' + job_id, session=session)[1]
            assert 'progress' not in retained, retained
            assert call('/candidates/' + candidate_id, session=session)[0] == 404
            code, refused = call('/generations', 'POST', session, body=payload)
            assert code == 404 and refused['error']['code'] == 'result_unavailable', (code, refused)
            assert (profile_home / 'runs').read_text() == '.'
            assert call('/candidates/' + candidate_id, session=session)[0] == 404
        (home / 'runs').write_text('')
        (home / 'partial').touch()
        code, accepted = call('/generations', 'POST', session, body=generation_input('partial', count=3))
        assert code == 202, (code, accepted)
        job_id = accepted['job']['id']
        assert wait_job(job_id)['state'] == 'succeeded'
        code, manifest = call('/generations/' + job_id, session=session)
        assert code == 200 and (manifest['progress']['ready_count'], manifest['progress']['failed_count']) == (2, 1), (code, manifest)
        assert (home / 'runs').read_text() == '...'
        assert call('/authentic/' + DATE + '/empty', 'POST', session, body={'operation_id': str(uuid.uuid4())})[0] == 200
        assert call('/generations/' + job_id, session=session)[1]['stale_baseline'] is True
        assert call('/candidates/' + manifest['candidates'][0]['id'], session=session)[0] == 200
        assert call('/jobs/' + job_id + '/result', 'DELETE', session)[0] == 200
        (home / 'partial').unlink()
        (home / 'scenario').write_text('late')
        code, accepted = call('/generations', 'POST', session, body=generation_input('cancel', count=2))
        assert code == 202, (code, accepted)
        job_id = accepted['job']['id']
        for _ in range(300):
            _, progress = call('/jobs/' + job_id, session=session)
            if progress['phase'] == 'waiting_for_output':
                break
            time.sleep(0.01)
        else:
            raise AssertionError('fake child did not start')
        assert call('/jobs/' + job_id + '/cancel', 'POST', session)[0] == 200
        assert wait_job(job_id)['state'] == 'cancelled'
        manifest = call('/generations/' + job_id, session=session)[1]
        assert (manifest['progress']['ready_count'], manifest['progress']['cancelled_count'], manifest['progress']['not_started_count']) == (0, 1, 1), manifest
        assert call('/generations/' + job_id, 'DELETE', session)[0] == 200
        runs_before = (home / 'runs').read_text()
        file.write_text(file.read_text() + 'changed\n')
        code, refused = call('/generations', 'POST', session, body=generation_input('changed-source'))
        assert code == 409 and refused['error']['code'] == 'reimport_required', (code, refused)
        assert (home / 'runs').read_text() == runs_before
        assert call('/reviews', 'POST', session)[0] == 404
        assert call('/candidates/not-a-candidate/accept', 'POST', session)[0] == 404
        traced = subprocess.run([str(binary), 'trace', '--config', str(config), '--select', 'recent:50'], capture_output=True, text=True, check=True).stdout
        for secret in [SOURCE_SECRET, 'Tea was nice.', str(base)]:
            assert secret not in traced, 'generation content/path leaked into diagnostics'
        assert '"action":"generate"' in traced and '"candidate_index":' in traced, traced
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
