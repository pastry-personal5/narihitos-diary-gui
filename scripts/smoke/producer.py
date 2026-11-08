"""P1-M14 producer round trip: GUI-saved files must decode as supported by the pinned mobile codec.

Builds the real diary-server and a Swift program
from the pinned codec files (swiftc, no dependency). Proves file format only: not the phone app's
catalog or recovery, not iCloud. Skips (exit 0 with SKIP) when the submodule or swiftc is missing;
a skip is not evidence. Run outside scripts/gate.sh.
"""
from isolation import server_configuration
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import uuid

root = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--schema2', action='store_true', help='use the captured manual-snippet producer codec and include schema 2 fixtures')
args = parser.parse_args()
mobile = root / 'contrib/narihitos-diary-mobile/NarihitosDiary'
codec = [mobile / 'Domain/DiarySnippet.swift', mobile / 'Adapters/DiarySnippetLayout.swift', mobile / 'Adapters/DiarySnippetMarkdownCodec.swift']
if args.schema2:
    codec = [root / 'scripts/smoke/producer/schema2' / p.name for p in codec]
if not shutil.which('swiftc') or not all(p.exists() for p in codec):
    print('SKIP producer round trip: swiftc or the pinned mobile submodule is missing (not evidence)')
    sys.exit(0)
DATE = '2024-02-29'
CREATED = '2024-02-29T12:00:00Z'
STEM_TIME = '2024-02-29_12-00-00'
BASE_FIELDS = [
    ('schema_version', '1'), ('kind', '"ndm.diary-snippet"'), ('id', None), ('created_at', f'"{CREATED}"'), ('updated_at', '"2024-02-29T12:00:00Z"'),
    ('title', '"Hello"'), ('title_origin', '"first-sentence"'), ('favorite', 'false'), ('language', '"en"'), ('audio_filename', None),
    ('recording_duration_ms', '1000'), ('audio_codec', '"aac"'), ('audio_sample_rate_hz', '44100'), ('audio_channels', '1'), ('audio_bitrate_bps', '64000'),
    ('transcription_method_id', '"ndm.transcription.apple"'), ('transcription_language_hint', '"en"'), ('transcription_settings_revision', '1'),
]


STEMS = {}


def document(body, overrides=None, extra=None, mutate=None, manual=False, time=None):
    ident = str(uuid.uuid4())
    fields = dict(BASE_FIELDS)
    fields['id'], fields['audio_filename'] = f'"{ident}"', f'"{ident}.m4a"'
    if manual:
        fields = {k: v for k, v in fields.items() if k in list(dict(BASE_FIELDS))[:8]}
        fields.update(schema_version='2', creation_method='"manual"')
    if time:
        fields['created_at'] = fields['updated_at'] = f'"2024-02-29T{time}Z"'
    STEMS[ident] = '2024-02-29_' + time.replace(':', '-') if time else STEM_TIME
    fields.update(overrides or {})
    lines = [f'{k}: {v}' for k, v in fields.items()] + (extra or [])
    text = '---\n' + '\n'.join(lines) + '\n---\n\n' + body
    if mutate:
        text = mutate(text)
    return ident, text.encode()


def rel(ident):
    return f'2024/02/29/{STEMS[ident]}-{ident}.md'


MATRIX = {
    'golden': dict(body='hello\n'),
    'unknown scalars': dict(body='x', extra=['unknown_flag: 7', 'unknown_real: 1.5', 'unknown_text: "kept"']),
    'location and weather': dict(body='x', extra=['location_latitude: 35.5', 'location_longitude: 139.25', 'location_sampled_at: "2024-02-29T12:00:00Z"', 'location_horizontal_accuracy_m: 5.0', 'location_place_name: "Tokyo"', 'weather_observed_at: "2024-02-29T12:00:00Z"', 'weather_summary: "Clear"', 'weather_temperature_celsius: 12.5']),
    'apple-ai title': dict(body='x', overrides={'title_origin': '"apple-ai"'}, extra=['title_prompt_revision: "1"', 'title_os_version: "26.0"']),
    'favorite': dict(body='x', overrides={'favorite': 'true'}),
    'updated_at :59': dict(body='x', overrides={'updated_at': '"2099-06-30T23:59:59Z"'}),
    'updated_at year end': dict(body='x', overrides={'updated_at': '"2099-12-31T23:59:59Z"'}),
    'unicode and emoji': dict(body='日本語 😀 é\n'),
    'empty body': dict(body=''),
    'no trailing newline': dict(body='no newline'),
    'trailing newline': dict(body='one\n'),
}
if args.schema2:
    MATRIX.update({
        'manual 01:42:07': dict(body='synthetic manual 日本語\n', manual=True, time='01:42:07', overrides={'title_origin': '"apple-ai"'}, extra=['title_prompt_revision: "1"', 'title_os_version: "26"', 'unknown_flag: 7']),
        'manual 01:42:57': dict(body='', manual=True, time='01:42:57', overrides={'title_origin': '"empty-body"'}),
        'manual favorite': dict(body='line one\nline two\n', manual=True, overrides={'favorite': 'true'}),
    })
NEW_BODIES = {'unicode and emoji': 'さようなら 🎉\n', 'empty body': 'now has text', 'no trailing newline': '', 'trailing newline': 'two\n\n'}

with tempfile.TemporaryDirectory(prefix='diary-producer-smoke-') as temporary:
    base = Path(temporary).resolve()
    harness = base / 'producer'
    subprocess.run(['swiftc', '-swift-version', '5', '-o', str(harness), str(root / 'scripts/smoke/producer/main.swift'), *map(str, codec)], check=True, capture_output=True)
    target = root / 'src-tauri/target/producer-harness'
    subprocess.run(['cargo', 'build', '--quiet', '--manifest-path', str(root / 'src-tauri/Cargo.toml'), '-p', 'diary-server', '--target-dir', str(target)], check=True)
    binary = target / 'debug/diary-server'

    def decode(path):
        out = subprocess.run([str(harness), 'decode', str(path)], capture_output=True, text=True)
        lines = out.stdout.strip().splitlines()
        assert out.returncode == 0 and lines and lines[0] == 'supported', (path.name, out.stdout, out.stderr)
        result = dict(line.split('=', 1) for line in lines[1:])
        result['body'] = base64.b64decode(result.pop('body_b64')).decode()
        return result

    workspace, runtime, source = base / 'workspace', base / 'runtime', base / 'source'
    for folder in [workspace, runtime, source]:
        folder.mkdir(mode=0o700)
    config = base / 'config.yaml'
    config.write_text(server_configuration() + f'schema_version: 1\npaths:\n  entries_root: {workspace}\n  runtime_root: {runtime}\nsources:\n  - {{alias: mobile, type: narihito, root: {source}}}\n')
    config.chmod(0o600)
    files, expected_before = {}, {}
    for name, spec in MATRIX.items():
        ident, data = document(**spec)
        path = source / rel(ident)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        files[name] = (ident, path)
        expected_before[name] = decode(path)
    big_id, big = document(body='é' * 524287 + 'x')  # 1 MiB - 1: the GUI reader counts the separator newline
    (source / rel(big_id)).write_bytes(big)
    files['1 MiB body'] = (big_id, source / rel(big_id))
    expected_before['1 MiB body'] = decode(files['1 MiB body'][1])

    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    processes = []

    def start():
        env = dict(os.environ)
        env.pop('DIARY_FAULT', None)
        process = subprocess.Popen([str(binary), '--config', str(config)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env)
        processes.append(process)
        for _ in range(500):
            for found in runtime.glob('*/discovery.json'):
                record = json.loads(found.read_text())
                if record['pid'] == process.pid:
                    return process, record
            assert process.poll() is None, 'server exited before readiness'
            time.sleep(0.02)
        raise AssertionError('server readiness timeout')

    def call(record, session, path, method='GET', payload=None):
        headers = {'Authorization': 'Bearer ' + record['token'], 'X-Diary-Session': session}
        if payload is not None:
            headers['Content-Type'] = 'application/json'
        request = urllib.request.Request(record['origin'] + '/api/v1' + path, data=None if payload is None else json.dumps(payload).encode(), headers=headers, method=method)
        try:
            with opener.open(request, timeout=20) as response:
                data = response.read()
                return response.status, json.loads(data) if data else None
        except urllib.error.HTTPError as error:
            return error.code, json.loads(error.read() or b'null')

    def session_for(record):
        request = urllib.request.Request(record['origin'] + '/api/v1/sessions', data=b'{}', headers={'Authorization': 'Bearer ' + record['token'], 'Content-Type': 'application/json'}, method='POST')
        with opener.open(request, timeout=5) as response:
            return json.loads(response.read())['session_id']

    def import_and_list(record, session, tag):
        code, ack = call(record, session, '/imports', 'POST', {'date': DATE, 'request_id': tag})
        assert code == 202, (code, ack)
        import_id = ack['import_id']
        for _ in range(500):
            code, job = call(record, session, '/jobs/' + import_id)
            if job['state'] in ('succeeded', 'failed', 'cancelled'):
                assert job['state'] == 'succeeded', job
                break
            time.sleep(0.02)
        found, cursor = {}, ''
        while True:
            code, page = call(record, session, f'/sources?import_id={import_id}' + cursor)
            assert code == 200, (code, page)
            for item in page['items']:
                if item['source'] is None:
                    print('non-source row:', {k: item[k] for k in item if k != 'source'})
                    continue
                found[item['source']['relative_key']] = item['source']
            if page['next_cursor'] is None:
                return import_id, found
            cursor = f"&cursor={page['next_cursor']}"

    def read(record, session, import_id, source_id):
        code, body = call(record, session, f'/sources/{source_id}?snapshot_id={import_id}')
        assert code == 200, (code, body)
        return body

    def save(record, session, source_id, base_digest, body, op=None):
        return call(record, session, f'/sources/{source_id}/body', 'PUT', {'operation_id': op or str(uuid.uuid4()), 'base_digest': base_digest, 'body': body})

    def sha(path):
        return hashlib.sha256(path.read_bytes()).hexdigest()

    try:
        server, record = start()
        session = session_for(record)
        import_id, listing = import_and_list(record, session, 'one')
        checked = {}
        for name, (ident, path) in files.items():
            assert rel(ident) in listing, ('not listed', name)
            item = listing[rel(ident)]
            assert item['eligible'] is True, (name, item)
            view = read(record, session, import_id, item['id'])
            assert view['capabilities']['save'] is True, (name, view['capabilities'])
            if name.startswith('manual'):
                assert view['capabilities']['reason'] == 'ready', (name, view['capabilities'])
                assert view['capabilities']['trash'] is False, (name, view['capabilities'])
            before = expected_before[name]
            new_body = NEW_BODIES.get(name, 'replaced ' + name + '\n')
            if name == '1 MiB body':
                new_body = 'ü' * 524287 + 'y'
            # The GUI view drops the producer's separator line; the saved body is exactly the text.
            assert view['body'] == before['body'], name
            code, out = save(record, session, item['id'], sha(path), new_body)
            assert code == 200 and out['digest'] == sha(path), (name, code, out)
            after = decode(path)
            assert after['body'] == new_body, name
            assert after['updated_at'] > before['updated_at'], (name, before['updated_at'], after['updated_at'])
            for key in ('title', 'title_origin', 'favorite', 'language', 'duration', 'preserved', 'created_at'):
                assert after[key] == before[key], (name, key, before[key], after[key])
            checked[name] = (item, out['digest'])
            # A second GUI save chains on the returned digest without a re-import.
            code, again = save(record, session, item['id'], out['digest'], new_body[:-1] + 'z' if name == '1 MiB body' else new_body + 'more')
            assert code == 200, (name, code, again)
            checked[name] = (item, again['digest'])
            assert decode(path)['updated_at'] > after['updated_at'], name
        # An equal body writes nothing.
        item, digest = checked['golden']
        path = files['golden'][1]
        stamp = path.stat().st_mtime_ns
        code, out = save(record, session, item['id'], digest, decode(path)['body'])
        assert code == 200 and out['unchanged'] is True and path.stat().st_mtime_ns == stamp, (code, out)
        # Producer-style edits are read back by the GUI, which then saves again.
        edit_body = base / 'edit.txt'
        edit_body.write_text('edited by the phone\n')
        producer_edits = [('unknown scalars', ['edit', None, str(edit_body), '2099-01-01T00:00:00Z']), ('favorite', ['toggle-favorite', None, '2099-01-01T00:00:00Z'])]
        if args.schema2:
            producer_edits += [('manual 01:42:07', ['edit', None, str(edit_body), '2099-01-01T00:00:00Z']), ('manual favorite', ['toggle-favorite', None, '2099-01-01T00:00:00Z'])]
        for name, command in producer_edits:
            path = files[name][1]
            command[1] = str(path)
            subprocess.run([str(harness), *command], check=True, capture_output=True)
        import_id, listing = import_and_list(record, session, 'two')
        for name, _ in producer_edits:
            ident, path = files[name]
            item = listing[rel(ident)]
            view = read(record, session, import_id, item['id'])
            assert view['body'] == decode(path)['body'], name
            code, out = save(record, session, item['id'], item['digest'], view['body'] + 'GUI\n')
            assert code == 200 and decode(path)['body'] == view['body'] + 'GUI\n', (name, code, out)
        # An external edit between open and save is refused and leaves the file alone.
        ident, path = files['golden']
        item = listing[rel(ident)]
        view = read(record, session, import_id, item['id'])
        subprocess.run([str(harness), 'edit', str(path), str(edit_body), '2099-02-01T00:00:00Z'], check=True, capture_output=True)
        theirs = path.read_bytes()
        code, out = save(record, session, item['id'], item['digest'], 'mine\n')
        assert code == 409 and out['error']['code'] == 'external_change' and path.read_bytes() == theirs, (code, out)
        if args.schema2:
            ident, path = files['manual 01:42:07']
            item = listing[rel(ident)]
            subprocess.run([str(harness), 'edit', str(path), str(edit_body), '2099-02-01T00:00:00Z'], check=True, capture_output=True)
            theirs = path.read_bytes()
            code, out = save(record, session, item['id'], item['digest'], 'mine\n')
            assert code == 409 and out['error']['code'] == 'external_change' and path.read_bytes() == theirs, (code, out)
        # Differential: whenever the GUI gate accepts a mutated document, the producer decodes it.
        mutations = {
            'bom': lambda t: '﻿' + t, 'crlf': lambda t: t.replace('\n', '\r\n'), 'no separator': lambda t: t.replace('\n---\n\n', '\n---\n', 1),
            'single quote': lambda t: t.replace('title: "Hello"', "title: 'Hello'"), 'list': lambda t: t.replace('favorite: false', 'favorite: [a]'),
            'bare word': lambda t: t.replace('favorite: false', 'favorite: maybe'), 'leading zero': lambda t: t.replace('audio_channels: 1', 'audio_channels: 01'),
            'plus': lambda t: t.replace('audio_channels: 1', 'audio_channels: +1'), 'minus zero': lambda t: t.replace('audio_channels: 1', 'audio_channels: -0'),
            'trailing dot': lambda t: t.replace('audio_channels: 1', 'audio_channels: 1.'), 'exponent': lambda t: t.replace('audio_channels: 1', 'audio_channels: 1e3'),
            'long decimal': lambda t: t.replace('favorite: false', 'favorite: false\nx: ' + '1' * 39 + '.5'), 'key with underscore start': lambda t: t.replace('favorite: false', 'favorite: false\n_x: 1'),
            'indent': lambda t: t.replace('favorite: false', 'favorite: false\n x: 1'), 'bad escape': lambda t: t.replace('"Hello"', '"a\\qb"'),
            'long title': lambda t: t.replace('"Hello"', '"' + 'x' * 31 + '"'), 'feb 30': lambda t: t.replace('"2024-02-29T12:00:00Z"', '"2023-02-29T12:00:00Z"', 1).replace('updated_at: "2023', 'updated_at: "2023'),
            'bad method': lambda t: t.replace('ndm.transcription.apple', 'ndm.transcription.other'), 'schema 2': lambda t: t.replace('schema_version: 1', 'schema_version: 2'),
            'apple-ai without provenance': lambda t: t.replace('first-sentence', 'apple-ai'), 'fractional updated': lambda t: t.replace('updated_at: "2024-02-29T12:00:00Z"', 'updated_at: "2024-02-29T12:00:00.5Z"'),
            'combining mark after quote': lambda t: t.replace('title: "Hello"', 'title: "́Hello"'), 'cjk title': lambda t: t.replace('"Hello"', '"日本語の題名"'),
            'valid decimal': lambda t: t.replace('favorite: false', 'favorite: false\nx: -12.25'), 'valid unknown string': lambda t: t.replace('favorite: false', 'favorite: false\nx: "q"'),
        }
        mutated = {}
        for name, mutate in mutations.items():
            ident, data = document(body='mutant\n', mutate=mutate)
            path = source / rel(ident)
            path.write_bytes(data)
            mutated[name] = (ident, path)
        import_id, listing = import_and_list(record, session, 'three')
        accepted = 0
        for name, (ident, path) in mutated.items():
            item = listing.get(rel(ident))
            if item is None:
                continue
            code, view = call(record, session, f"/sources/{item['id']}?snapshot_id={import_id}")
            if code != 200 or not view['capabilities']['save']:
                continue
            accepted += 1
            out = subprocess.run([str(harness), 'decode', str(path)], capture_output=True, text=True)
            assert out.stdout.startswith('supported'), f'gate accepted a document the producer rejects: {name}: {out.stdout}'
            code, saved = save(record, session, item['id'], item['digest'], 'mutant 2\n')
            assert code == 200, (name, code, saved)
            assert decode(path)['body'] == 'mutant 2\n', name
        assert accepted >= 2, accepted
        print(f'PASS producer round trip ({len(files)} fixtures, {accepted} accepted mutants)')
    finally:
        for process in processes:
            if process.poll() is None:
                process.send_signal(signal.SIGINT)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
