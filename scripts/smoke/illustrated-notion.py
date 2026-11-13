#!/usr/bin/env python3
"""Isolated Illustrated Notion upload, with optional authorized test-root-only live mode.

Default uses fault-injection FakeNotion; --live requires DIARY_NOTION_LIVE=1 and
reads only the token from --config. Real test pages are retained, never archived.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import sqlite3
import struct
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request
import uuid
import zlib
from isolation import server_configuration

ROOT = Path(__file__).resolve().parents[2]


def fixture_png():
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 2, 1, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(b'\0\x20\x60\xc0\xff\xff\x80\x20\xff')) + chunk(b'IEND', b''))


def seed(workspace, date, text):
    version = str(uuid.uuid4())
    parent = workspace / 'assets/diary/illustrated' / date[:4] / date[5:7]
    images = parent / date
    images.mkdir(parents=True, exist_ok=True, mode=0o700)
    png = fixture_png()
    photo = images / '01.png'
    photo.write_bytes(png)
    photo.chmod(0o600)
    source = str(uuid.uuid4())
    header = ('---\nschema_version: 1\nasset_type: diary\nkind: illustrated\n'
              f'id: "{version}"\ndiary_date: "{date}"\ncreated_at: "2026-11-13T00:00:00Z"\n'
              f'source_authentic_id: "{source}"\nsource_revision: 1\nsource_sha256: "' + 'a' * 64 + '"\n'
              'agent_profile: "not-configured"\nimage_engine: "not-configured"\ntrigger: "private-trigger"\n'
              'prompt_directives:\n  - "private-directive"\ntheme_count: 1\nphotos:\n'
              '  - file: "01.png"\n    seed: 17\n    prompt: "private-trigger private-prompt"\n---\n')
    markdown = parent / (date + '.md')
    markdown.write_text(header + text + f'\n\n![Fixture 한😀]({date}/01.png)\n\nAfter photo.\n')
    markdown.chmod(0o600)
    return version, {path: hashlib.sha256(path.read_bytes()).hexdigest() for path in [markdown, photo]}


class Server:
    def __init__(self, binary, config, runtime, env, log):
        self.process = subprocess.Popen([str(binary), '--config', str(config)], env=env,
                                        stdout=subprocess.DEVNULL, stderr=log)
        self.stop_heartbeat = threading.Event()
        try:
            self.ready(runtime)
        except BaseException:
            self.close()
            raise

    def ready(self, runtime):
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise RuntimeError('isolated server exited before readiness')
            for path in runtime.glob('*/discovery.json'):
                record = json.loads(path.read_text())
                if record['pid'] == self.process.pid:
                    self.record = record
                    self.session = self.call('/sessions', 'POST', {}, session=False)[1]['session_id']
                    self.thread = threading.Thread(target=self.heartbeat, daemon=True)
                    self.thread.start()
                    return
            time.sleep(0.05)
        raise RuntimeError('isolated server readiness timeout')

    def call(self, path, method='GET', body=None, session=True):
        headers = {'Authorization': 'Bearer ' + self.record['token'], 'Content-Type': 'application/json'}
        if session:
            headers['X-Diary-Session'] = self.session
        request = urllib.request.Request(self.record['origin'] + '/api/v1' + path, method=method,
                                         headers=headers, data=None if body is None else json.dumps(body).encode())
        try:
            response = urllib.request.build_opener(urllib.request.ProxyHandler({})).open(request, timeout=120)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            data = response.read()
            return response.status, json.loads(data) if data else None

    def heartbeat(self):
        while not self.stop_heartbeat.wait(5):
            try:
                self.call('/sessions/' + self.session + '/heartbeat', 'POST')
            except OSError:
                return

    def close(self):
        self.stop_heartbeat.set()
        if self.process.poll() is None:
            self.process.send_signal(signal.SIGINT)
            try:
                self.process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)

    def upload(self, date, version):
        operation = str(uuid.uuid4())
        status, result = self.call('/exports', 'POST', {'operation_id': operation, 'date': date,
            'target': 'notion', 'content_kind': 'illustrated', 'expected_version': version})
        if status != 200:
            raise RuntimeError('upload admission failed: ' + str(status) + ' ' + str((result or {}).get('error', {}).get('code')))
        if result.get('outcome') == 'needs_confirmation':
            raise RuntimeError('unexpected confirmation in fresh isolated fixture')
        deadline = time.monotonic() + 300
        while time.monotonic() < deadline:
            status, record = self.call('/operations/' + operation)
            if status != 200:
                raise RuntimeError('operation lookup failed')
            export = record['export']
            if export['state'] == 'succeeded':
                return export
            if export['state'] != 'running':
                raise RuntimeError('upload stopped: ' + export['state'] + ' ' + str(export.get('reason')))
            time.sleep(0.2)
        raise RuntimeError('upload did not settle')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--live', action='store_true')
    parser.add_argument('--gui-app', type=Path)
    parser.add_argument('--config', type=Path, default=ROOT / 'config/global_config.yaml')
    args = parser.parse_args()
    if args.live and args.gui_app:
        parser.error('--gui-app is synthetic only')
    if args.live and os.environ.get('DIARY_NOTION_LIVE') != '1':
        parser.error('live mode requires DIARY_NOTION_LIVE=1')
    target = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'src-tauri/target')).resolve()
    build_target = target if args.live else target / 'fault'
    env = dict(os.environ, CARGO_TARGET_DIR=str(build_target))
    command = ['cargo', 'build', '--quiet', '--manifest-path', str(ROOT / 'src-tauri/Cargo.toml'), '-p', 'diary-server']
    if not args.live:
        command += ['--features', 'fault-injection']
    subprocess.run(command, check=True, env=env)
    probe = None
    destination = '0123456789abcdef0123456789abcdef'
    token = 'synthetic-token'
    if args.live:
        spec = importlib.util.spec_from_file_location('qualification', Path(__file__).with_name('notion-page-qualification.py'))
        qualification = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(qualification)
        token = qualification.private_token(args.config)
        probe = qualification.Probe(token)
        status, _ = probe.request('GET', 'pages/' + qualification.ROOT)
        if status != 200:
            raise RuntimeError('authorized test root is not accessible')
        status, page = probe.create(qualification.ROOT, 'Illustrated upload smoke ' + datetime.now(timezone.utc).isoformat())
        if status != 200:
            raise RuntimeError('test container creation failed; do not replay blindly')
        destination = page['id']
        print('retained live fixture root:', destination, flush=True)
    with tempfile.TemporaryDirectory(prefix='illustrated-notion-') as temporary:
        base = Path(temporary).resolve()
        workspace, runtime = base / 'workspace', base / 'runtime'
        for folder in [workspace, runtime]:
            folder.mkdir(mode=0o700)
        config = base / 'config.yaml'
        config.write_text(server_configuration() + f'schema_version: 1\npaths:\n  entries_root: {workspace}\n  runtime_root: {runtime}\nnotion:\n  token: "{token}"\n  pages:\n    - type: illustrated\n      root_page_id: "{destination}"\n')
        config.chmod(0o600)
        if not args.live:
            env['DIARY_FAKE_NOTION_FILE'] = str(base / 'notion.json')
        server = None
        with (base / 'stderr.log').open('w') as log:
            try:
                server = Server(build_target / 'debug/diary-server', config, runtime, env, log)
                status, settings = server.call('/settings')
                assert status == 200 and settings['illustrated_notion_exports_available'] and not settings['notion_exports_available']
                dates = ['2036-05-15', '2032-01-01', '2038-01-01', '2034-01-01', '2036-01-01', '2036-03-01', '2036-12-01', '2036-05-01', '2036-05-10', '2036-05-30']
                for date in dates:
                    version, originals = seed(workspace, date, 'Fixture text 한😀.')
                    result = server.upload(date, version)
                    assert result['content_kind'] == 'illustrated' and result['illustrated_version'] == version
                    for path, sha in originals.items():
                        assert hashlib.sha256(path.read_bytes()).hexdigest() == sha
                    print('uploaded fixture date:', date, flush=True)
                date = dates[0]
                version = server.call('/illustrated/' + date)[1]['version']
                def receipt():
                    with sqlite3.connect(workspace / '.diary/state.sqlite3') as db:
                        return db.execute('SELECT remote_ids,remote_fingerprint FROM export_receipts WHERE date=?', (date,)).fetchone()
                before = receipt()
                server.upload(date, version)
                assert receipt() == before
                version, originals = seed(workspace, date, 'Replacement text 한😀.')
                server.upload(date, version)
                after = receipt()
                assert after[0] == before[0] and after[1] != before[1]
                server.close()
                server = Server(build_target / 'debug/diary-server', config, runtime, env, log)
                server.upload(date, version)
                assert receipt() == after
                for path, sha in originals.items():
                    assert hashlib.sha256(path.read_bytes()).hexdigest() == sha
                with sqlite3.connect(workspace / '.diary/state.sqlite3') as db:
                    assert db.execute('SELECT count(*) FROM export_photos').fetchone()[0] == 0
                if args.gui_app:
                    today = datetime.now().astimezone().date().isoformat()
                    seed(workspace, today, 'Packaged upload fixture.')
                    server.stop_heartbeat.set()
                    assert server.call('/sessions/' + server.session, 'DELETE')[0] == 204
                    settings_dir = base / 'gui-settings'
                    settings_dir.mkdir(mode=0o700)
                    client = args.gui_app.resolve(strict=True) / 'Contents/MacOS/diary-client'
                    gui_env = dict(os.environ, DIARY_ILLUSTRATED_UPLOAD_GUI_SMOKE='1', DIARY_ILLUSTRATED_SMOKE_SETTINGS=str(settings_dir))
                    gui_log = base / 'gui.log'
                    with gui_log.open('w') as output:
                        gui = subprocess.Popen([str(client), '--connect', str(next(runtime.glob('*/discovery.json')))], env=gui_env, stdout=output, stderr=output)
                        try:
                            gui.wait(timeout=180)
                        finally:
                            if gui.poll() is None:
                                gui.terminate()
                                try:
                                    gui.wait(timeout=5)
                                except subprocess.TimeoutExpired:
                                    gui.kill()
                                    gui.wait(timeout=5)
                    lines = gui_log.read_text().splitlines()
                    reports = [line for line in lines if 'SMOKE illustrated_gui ' in line and ('passed' in line or 'failed' in line)]
                    if gui.returncode != 0 or not any('passed' in line for line in reports):
                        print('native fixture reports:', reports, flush=True)
                        raise RuntimeError('packaged upload GUI failed')
                    print('PASS packaged Illustrated upload GUI:', reports[-1], flush=True)
                if probe:
                    status, body = probe.request('GET', 'blocks/' + after[0] + '/children')
                    assert status == 200
                    blocks = body['results']
                    assert [b['type'] for b in blocks] == ['paragraph', 'image', 'paragraph']
                    assert 'private-' not in json.dumps(blocks)
                    jpeg = probe.opener.open(blocks[1]['image']['file']['url'], timeout=30).read(5 * 1024 * 1024 + 1)
                    assert jpeg.startswith(b'\xff\xd8') and len(jpeg) < 5 * 1024 * 1024
                    print('retained stable day page:', after[0], 'JPEG bytes:', len(jpeg), flush=True)
                print('PASS Illustrated Notion create/no-op/backdated hierarchy/stable-page replace/restart/originals/cleanup', flush=True)
            finally:
                if server:
                    server.close()


if __name__ == '__main__':
    main()
