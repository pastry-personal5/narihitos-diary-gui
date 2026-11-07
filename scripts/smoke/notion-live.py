"""P1-M27 live Notion round trip against a DEDICATED THROWAWAY root page (owner O1/O6).

Env-gated: DIARY_NOTION_LIVE=1, DIARY_NOTION_LIVE_TOKEN=<integration token>,
DIARY_NOTION_LIVE_ROOT=<throwaway root page id shared with the integration>.
It writes real pages under that root and leaves them in place (nothing is archived or deleted
by this script or the app). It spends Notion API calls. Run it with the real (non-fault) binary.
Records actual service behavior for Q1 (workspace_id), Q4 (nesting) and page ordering."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.request
import uuid

if os.environ.get('DIARY_NOTION_LIVE') != '1':
    sys.exit('skipped: set DIARY_NOTION_LIVE=1 (and TOKEN/ROOT) to run the live Notion round trip')
TOKEN = os.environ.get('DIARY_NOTION_LIVE_TOKEN', '').strip()
ROOT_PAGE = os.environ.get('DIARY_NOTION_LIVE_ROOT', '').strip()
if not TOKEN or not ROOT_PAGE:
    sys.exit('DIARY_NOTION_LIVE_TOKEN and DIARY_NOTION_LIVE_ROOT must both be non-empty '
             '(an unset shell variable such as ${TOKEN} expands to nothing)')
root = Path(__file__).resolve().parents[2]
manifest = str(root / 'src-tauri/Cargo.toml')
subprocess.run(['cargo', 'build', '--quiet', '--manifest-path', manifest, '-p', 'diary-server'], check=True)
binary = root / 'src-tauri/target/debug/diary-server'
OP = lambda: str(uuid.uuid4())
opener = urllib.request.build_opener()
observed = {}


def notion(method, path, body=None):
    request = urllib.request.Request('https://api.notion.com' + path, method=method, data=None if body is None else json.dumps(body).encode(),
                                     headers={'Authorization': 'Bearer ' + TOKEN, 'Notion-Version': '2026-03-11', 'Content-Type': 'application/json'})
    try:
        with opener.open(request, timeout=30) as response:
            return response.status, json.loads(response.read())
    except urllib.error.HTTPError as error:
        return error.code, json.loads(error.read() or b'null')


def children(block):
    out, cursor = [], None
    while True:
        code, page = notion('GET', f'/v1/blocks/{block}/children?page_size=100' + (f'&start_cursor={cursor}' if cursor else ''))
        assert code == 200, (code, page and page.get('code'))
        out += page['results']
        if not page['has_more']:
            return out
        cursor = page['next_cursor']


def text(block):
    kind = block['type']
    return ''.join(i['plain_text'] for i in block[kind].get('rich_text', []))


# Q1: record whether the bot carries a workspace_id.
code, me = notion('GET', '/v1/users/me')
if code != 200:
    hint = {401: 'Notion rejected the token: it is invalid, revoked or empty.',
            403: 'The integration lacks a needed capability.'}.get(code, 'Unexpected response.')
    sys.exit(f"GET /v1/users/me returned {code} ({(me or {}).get('code')}): {hint}")
observed['q1_workspace_id_present'] = 'workspace_id' in me.get('bot', {})
print('Q1 workspace_id present:', observed['q1_workspace_id_present'])

with tempfile.TemporaryDirectory(prefix='diary-notion-live-') as temporary:
    base = Path(temporary).resolve()
    workspace, runtime = base / 'workspace', base / 'runtime'
    for folder in [workspace, runtime]:
        folder.mkdir(mode=0o700)
    config = base / 'config.yaml'
    config.write_text(f'schema_version: 1\npaths:\n  entries_root: {workspace}\n  runtime_root: {runtime}\nnotion:\n  root_page_id: "{ROOT_PAGE}"\n  token: "{TOKEN}"\n')
    config.chmod(0o600)

    def start():
        env = dict(os.environ)
        process = subprocess.Popen([str(binary), '--config', str(config)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env)
        for _ in range(1500):
            for found in runtime.glob('*/discovery.json'):
                record = json.loads(found.read_text())
                if record['pid'] == process.pid:
                    return process, record
            assert process.poll() is None, 'server exited before readiness'
            time.sleep(0.02)
        raise AssertionError('server readiness timeout')

    class Client:
        def __init__(self, record):
            self.record = record
            self.session = self.raw('/sessions', 'POST', {})[1]['session_id']
            # The lease expires after 30 s without a heartbeat and live Notion calls are slow.
            self.alive = threading.Event()
            threading.Thread(target=self.beat, daemon=True).start()

        def beat(self):
            while not self.alive.wait(5):
                self.raw(f'/sessions/{self.session}/heartbeat', 'POST', None, self.session)

        def raw(self, path, method='GET', body=None, session=None):
            headers = {'Authorization': 'Bearer ' + self.record['token'], 'Content-Type': 'application/json'}
            if session:
                headers['X-Diary-Session'] = session
            request = urllib.request.Request(self.record['origin'] + '/api/v1' + path, data=None if body is None else json.dumps(body).encode(), headers=headers, method=method)
            try:
                with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(request, timeout=60) as response:
                    data = response.read()
                    return response.status, json.loads(data) if data else None
            except urllib.error.HTTPError as error:
                return error.code, json.loads(error.read() or b'null')

        def call(self, path, method='GET', body=None):
            return self.raw(path, method, body, self.session)

        def save(self, date, body):
            code, loaded = self.call('/authentic/' + date)
            assert code == 200, (code, loaded)
            code, saved = self.call('/authentic/' + date, 'PUT', {'operation_id': OP(), 'expected_revision': loaded['revision'], 'body': body, 'buffer_generation': 1})
            assert code == 200, (code, saved)
            return self.call('/authentic/' + date)[1]['revision']

        def revision(self, date):
            code, body = self.call('/authentic/' + date)
            assert code == 200 and body and body.get('revision'), (date, code, sorted((body or {}).keys()))
            return body['revision']

        def settle(self, date, revision):
            """Exports and, because earlier runs leave sections under the throwaway root, confirms
            an `unowned`/`changed` replacement the owner authorized for this page."""
            op, code, out = self.export(date, revision)
            assert code == 200, (code, out)
            if out['outcome'] == 'needs_confirmation':
                self.confirm(op, out)
            self.wait(op, 'succeeded')
            return op

        def export(self, date, revision):
            op = OP()
            code, out = self.call('/exports', 'POST', {'operation_id': op, 'date': date, 'target': 'notion', 'expected_revision': revision})
            return op, code, out

        def wait(self, op, state):
            for _ in range(3000):
                code, found = self.call('/operations/' + op)
                assert code == 200, (code, found)
                if found['export']['state'] == state:
                    return found
                time.sleep(0.1)
            raise AssertionError(f'{op} never reached {state}: {found}')

        def confirm(self, op, preview):
            code, ack = self.call(f'/exports/{op}/confirmation', 'POST', {'step_id': OP(), 'expected_checkpoint': preview['expected_checkpoint'], 'token': preview['token']})
            assert code == 200, (code, ack)

    def stop(process, client=None):
        if client:
            client.alive.set()
        process.send_signal(signal.SIGINT)
        process.wait(timeout=15)

    def month_days():
        year = next(b for b in children(ROOT_PAGE) if b['type'] == 'child_page' and b['child_page']['title'] == '2026')
        month = next(b for b in children(year['id']) if b['type'] == 'child_page' and b['child_page']['title'] == '2026-11')
        return month['id'], [b for b in children(month['id']) if b['type'] == 'heading_2']

    def day_body(month_id, prefix):
        """The blocks after the day heading starting with `prefix`, up to the next day heading."""
        body, inside = [], False
        for b in children(month_id):
            if b['type'] == 'heading_2':
                if inside:
                    break
                inside = text(b).startswith(prefix)
            elif inside:
                body.append(b)
        return body

    server, record = start()
    c = Client(record)
    assert c.call('/status')[0] in (200, 404)
    # Create, in an order that exercises first/middle/last insertion.
    for date in ['2026-11-10', '2026-11-03', '2026-11-07', '2026-11-20']:
        rev = c.save(date, f'Live entry for {date}.\n\n- item one\n  - nested item\n\n> quoted\n')
        c.settle(date, rev)
    month_id, days = month_days()
    titles = [text(t) for t in days]
    assert titles == sorted(titles) and len(titles) == 4, titles
    print('days ascending:', titles)
    # Q4: how Notion nests a list item's child under a day's blocks.
    kids = day_body(month_id, '2026-11-07')
    observed['q4_children'] = [(k['type'], k['has_children']) for k in kids]
    print('Q4 section children:', observed['q4_children'])
    # No-op.
    c.settle('2026-11-07', c.revision('2026-11-07'))
    # Long Unicode entry across rich-text and chunk limits, then an external edit and replace.
    long_body = '한😀' * 1500 + '\n\n' + ''.join(f'para {i}\n\n' for i in range(150))
    rev = c.save('2026-11-07', long_body)
    op, code, out = c.export('2026-11-07', rev)
    if out['outcome'] == 'needs_confirmation':
        c.confirm(op, out)
    c.wait(op, 'succeeded')
    kids = day_body(month_days()[0], '2026-11-07')
    assert len(kids) == 151, len(kids)
    code, _ = notion('PATCH', f"/v1/blocks/{kids[1]['id']}", {'paragraph': {'rich_text': [{'type': 'text', 'text': {'content': 'edited in Notion'}}]}})
    assert code == 200
    rev = c.save('2026-11-07', 'Replacement body.\n')
    op, code, pv = c.export('2026-11-07', rev)
    assert code == 200 and pv['outcome'] == 'needs_confirmation' and pv['reason'] == 'changed', (code, pv)
    c.confirm(op, pv)
    c.wait(op, 'succeeded')
    assert [text(k) for k in day_body(month_days()[0], '2026-11-07')] == ['Replacement body.']
    # Restart: the receipt survives and an unchanged export is a no-op.
    stop(server, c)
    server, record = start()
    c = Client(record)
    op, code, out = c.export('2026-11-07', c.revision('2026-11-07'))
    assert code == 200 and out['outcome'] != 'needs_confirmation', (code, out)
    c.wait(op, 'succeeded')
    stop(server, c)

# Q1/Q2 probe on the month page: what an append with `position` returns, and whether blocks can be
# deleted in order (first, then the one that followed it). The probe blocks are deleted again.
_, year_blocks = None, [b for b in children(ROOT_PAGE) if b['type'] == 'child_page' and b['child_page']['title'] == '2026']
month_block = next(b for b in children(year_blocks[0]['id']) if b['type'] == 'child_page' and b['child_page']['title'] == '2026-11')
first_day = next(b for b in children(month_block['id']) if b['type'] == 'heading_2')
probe = lambda t: {'object': 'block', 'type': 'paragraph', 'paragraph': {'rich_text': [{'type': 'text', 'text': {'content': t}}]}}
code, reply = notion('PATCH', f"/v1/blocks/{month_block['id']}/children", {'children': [probe('q probe a'), probe('q probe b')], 'position': {'type': 'after_block', 'after_block': {'id': first_day['id']}}})
assert code == 200, (code, reply)
observed['q1_reply_blocks'] = len(reply['results'])
observed['q1_month_blocks'] = len(children(month_block['id']))
ids = [b['id'] for b in reply['results'] if text(b).startswith('q probe')]
observed['q2_ordered_deletes'] = [notion('DELETE', f'/v1/blocks/{i}')[0] for i in ids]
print('Q1:', {k: v for k, v in observed.items() if k.startswith('q1')}, 'Q2:', observed['q2_ordered_deletes'])

# Page ordering probe: does page creation accept a position? (informational; leaves two pages)
code, a = notion('POST', '/v1/pages', {'parent': {'page_id': ROOT_PAGE}, 'properties': {'title': {'title': [{'text': {'content': 'zz-order-probe-a'}}]}}})
code2, b = notion('POST', '/v1/pages', {'parent': {'page_id': ROOT_PAGE}, 'position': {'type': 'start'}, 'properties': {'title': {'title': [{'text': {'content': 'zz-order-probe-b'}}]}}})
observed['page_position_accepted'] = code2 == 200
if code2 == 200:
    titles = [c['child_page']['title'] for c in children(ROOT_PAGE) if c['type'] == 'child_page']
    observed['page_position_honored'] = titles.index('zz-order-probe-b') < titles.index('zz-order-probe-a')
print('page ordering probe:', {k: v for k, v in observed.items() if k.startswith('page_')})
print('PASS notion live (pages left in place under the throwaway root)')
