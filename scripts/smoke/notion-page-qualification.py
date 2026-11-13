"""Opt-in positional page creation probe, restricted to the authorized test root.

Run through notion-live.py --qualify-pages --config config/global_config.yaml.
Requires DIARY_NOTION_LIVE=1. Retains every created page. Never retries writes.
Only reads the token from config; configured destination roots are never used.
"""
import argparse
from datetime import datetime, timezone
import json
import hashlib
import os
from pathlib import Path
import re
import stat
import sys
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import urllib.parse
import uuid

ROOT = '417de2b705cc80239abdd1fc1654154f'
VERSION = '2026-03-11'


def private_token(path):
    """Read only the conventional literal token scalar, without echoing config."""
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o600:
        raise ValueError('config must be an owner-only regular file')
    if info.st_size > 256 * 1024:
        raise ValueError('config exceeds size limit')
    source = path.read_text()
    sections = re.findall(r'^notion:\s*\n((?:[ \t]+[^\n]*\n|\n)*)', source + '\n', re.M)
    if len(sections) != 1:
        raise ValueError('expected one notion mapping')
    tokens = re.findall(r'^  token: *([\"\']?)([A-Za-z0-9_-]+)\1 *(?:#.*)?$', sections[0], re.M)
    if len(tokens) != 1:
        raise ValueError('expected one literal notion.token scalar')
    return tokens[0][1]


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


class Probe:
    def __init__(self, token):
        self.token = token
        self.opener = urllib.request.build_opener(NoRedirect())

    def request(self, method, path, body=None, content_type='application/json'):
        time.sleep(0.4)
        request = urllib.request.Request(
            'https://api.notion.com/v1/' + path, method=method,
            data=body if isinstance(body, bytes) else None if body is None else json.dumps(body).encode(),
            headers={'Authorization': 'Bearer ' + self.token,
                     'Notion-Version': VERSION, 'Content-Type': content_type})
        try:
            with self.opener.open(request, timeout=30) as response:
                return response.status, json.loads(response.read(1024 * 1024))
        except urllib.error.HTTPError as error:
            data = json.loads(error.read(1024 * 1024))
            # Only allowlisted service codes; response bodies never reach logs.
            return error.code, {'code': data.get('code') if data.get('code') in {
                'validation_error', 'unauthorized', 'restricted_resource', 'object_not_found',
                'rate_limited', 'internal_server_error', 'service_unavailable'} else 'other'}

    def children(self, parent):
        result, cursor = [], None
        while True:
            status, page = self.request('GET', 'blocks/' + parent + '/children?page_size=100'
                                        + ('&start_cursor=' + cursor if cursor else ''))
            if status != 200:
                raise RuntimeError('child read failed: HTTP ' + str(status))
            result.extend(page['results'])
            if not page['has_more']:
                return result
            cursor = page['next_cursor']

    def create(self, parent, title, position=None):
        body = {'parent': {'page_id': parent}, 'properties': {
            'title': {'title': [{'type': 'text', 'text': {'content': title}}]}}}
        if position is not None:
            body['position'] = position
        return self.request('POST', 'pages', body)


def fixture_jpeg(red=False):
    """Produce synthetic pixels with system ImageIO, without a new dependency."""
    with tempfile.TemporaryDirectory(prefix='notion-fixture-') as temporary:
        output = Path(temporary) / 'fixture.jpg'
        source = '''import Foundation
import CoreGraphics
import ImageIO
let destination = URL(fileURLWithPath: CommandLine.arguments[1])
let context = CGContext(data: nil, width: 640, height: 480, bitsPerComponent: 8,
    bytesPerRow: 640 * 4, space: CGColorSpaceCreateDeviceRGB(),
    bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)!
context.setFillColor(CGColor(red: 0.15, green: 0.45, blue: 0.85, alpha: 1))
context.fill(CGRect(x: 0, y: 0, width: 640, height: 480))
let encoder = CGImageDestinationCreateWithURL(destination as CFURL, "public.jpeg" as CFString, 1, nil)!
CGImageDestinationAddImage(encoder, context.makeImage()!,
    [kCGImageDestinationLossyCompressionQuality: 0.85] as CFDictionary)
if !CGImageDestinationFinalize(encoder) { exit(1) }
'''
        if red:
            source = source.replace('red: 0.15, green: 0.45, blue: 0.85', 'red: 0.85, green: 0.15, blue: 0.25')
        program = Path(temporary) / 'fixture.swift'
        program.write_text(source)
        subprocess.run(['swift', str(program), str(output)], check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=60)
        return output.read_bytes()


def require_test_container(probe, parent):
    parent = str(uuid.UUID(parent))
    status, page = probe.request('GET', 'pages/' + parent)
    if status != 200 or page.get('parent', {}).get('page_id', '').replace('-', '') != ROOT:
        raise ValueError('image probe requires a direct child of the authorized root')
    return parent


def image_evidence(block):
    image = block['image']
    url = image.get('file', {}).get('url', '')
    return {'id': block['id'], 'last_edited_time': block['last_edited_time'],
            'caption_sha256': hashlib.sha256(json.dumps(image.get('caption', []), sort_keys=True).encode()).hexdigest(),
            'image_type': image['type'], 'image_keys': sorted(image),
            'file_keys': sorted(image.get('file', {})),
            'file_path_sha256': hashlib.sha256(urllib.parse.urlsplit(url).path.encode()).hexdigest()}


def observe_images(probe, parent):
    result = [image_evidence(block) for block in probe.children(parent) if block['type'] == 'image']
    print(json.dumps({'image_observation': result}, sort_keys=True), flush=True)


def download_image_hash(opener, url):
    """Qualification only: bounded hosted-file read, no credentials or redirects."""
    parsed = urllib.parse.urlsplit(url)
    if (parsed.scheme != 'https' or parsed.username or parsed.password
            or parsed.port not in (None, 443)
            or not parsed.hostname or not (
                parsed.hostname.endswith('.amazonaws.com') or parsed.hostname == 'file.notion.so')):
        raise ValueError('unsupported hosted file URL')
    limit = 20 * 1024 * 1024
    with opener.open(urllib.request.Request(url, method='GET'), timeout=30) as response:
        if response.headers.get('Content-Length') and int(response.headers['Content-Length']) > limit:
            raise ValueError('image exceeds qualification byte limit')
        data = response.read(limit + 1)
        if len(data) > limit:
            raise ValueError('image exceeds qualification byte limit')
    return {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}


def qualify_image_bytes(probe, parent):
    result = []
    for block in probe.children(parent):
        if block['type'] != 'image' or block['image']['type'] != 'file':
            continue
        result.append({'id': block['id'], **download_image_hash(probe.opener, block['image']['file']['url'])})
    print(json.dumps({'hosted_image_bytes': result, 'local_fixture_bytes': {
        'blue': hashlib.sha256(fixture_jpeg()).hexdigest(),
        'red': hashlib.sha256(fixture_jpeg(red=True)).hexdigest()}}), flush=True)


def upload_fixture(probe, jpeg):
    status, upload = probe.request('POST', 'file_uploads',
                                   {'filename': 'fixture.jpg', 'content_type': 'image/jpeg'})
    if status != 200:
        raise RuntimeError('upload creation failed')
    upload_id = str(uuid.UUID(upload['id']))
    print('fixture upload:', upload_id, flush=True)
    boundary = 'diary-fixture-boundary'
    multipart = (('--' + boundary + '\r\nContent-Disposition: form-data; name="file"; filename="fixture.jpg"\r\n'
                  'Content-Type: image/jpeg\r\n\r\n').encode() + jpeg
                 + ('\r\n--' + boundary + '--\r\n').encode())
    status, sent = probe.request('POST', 'file_uploads/' + upload_id + '/send', multipart,
                                 'multipart/form-data; boundary=' + boundary)
    if status != 200 or sent.get('status') != 'uploaded':
        raise RuntimeError('upload send failed; do not replay blindly')
    return upload_id


def prepare_images(probe, parent):
    upload_id = upload_fixture(probe, fixture_jpeg())
    labels = ['Replace this image in Notion', 'Change this caption in Notion', 'Delete this image in Notion']
    blocks = [{'object': 'block', 'type': 'image', 'image': {
        'type': 'file_upload', 'file_upload': {'id': upload_id},
        'caption': [{'type': 'text', 'text': {'content': label}}]}} for label in labels]
    status, reply = probe.request('PATCH', 'blocks/' + parent + '/children', {'children': blocks})
    if status != 200:
        raise RuntimeError('attach failed; do not replay blindly')
    for label, block in zip(labels, reply['results']):
        print(label + ':', block['id'], flush=True)
    observe_images(probe, parent)
    print('WAITING: edit the three fixture images in Notion, then run --observe-images ' + parent, flush=True)


def rapid_image_edit(probe, parent):
    """An image can change between two observations inside one timestamp minute."""
    blue = upload_fixture(probe, fixture_jpeg())
    red = upload_fixture(probe, fixture_jpeg(red=True))
    caption = [{'type': 'text', 'text': {'content': 'Same-minute image replacement probe'}}]
    status, reply = probe.request('PATCH', 'blocks/' + parent + '/children', {'children': [{
        'object': 'block', 'type': 'image', 'image': {
            'type': 'file_upload', 'file_upload': {'id': blue}, 'caption': caption}}]})
    if status != 200:
        raise RuntimeError('append failed; do not replay blindly')
    block_id = str(uuid.UUID(reply['results'][0]['id']))
    status, before = probe.request('GET', 'blocks/' + block_id)
    if status != 200:
        raise RuntimeError('baseline read failed')
    status, _ = probe.request('PATCH', 'blocks/' + block_id,
                               {'image': {'file_upload': {'id': red}}})
    print('same-minute image replacement HTTP:', status, flush=True)
    if status != 200:
        raise RuntimeError('image replacement failed; do not replay blindly')
    status, after = probe.request('GET', 'blocks/' + block_id)
    if status != 200:
        raise RuntimeError('replacement read failed')
    status, original = probe.request('GET', 'file_uploads/' + blue)
    if status != 200:
        raise RuntimeError('original upload read failed')
    old, new = image_evidence(before), image_evidence(after)
    print(json.dumps({'rapid_edit_before': old, 'rapid_edit_after': new,
                      'original_upload_status_after_replacement': original.get('status')}), flush=True)
    keys = ['id', 'last_edited_time', 'caption_sha256']
    if all(old[key] == new[key] for key in keys) and old['file_path_sha256'] != new['file_path_sha256']:
        print('FAIL drift evidence: image changed without changing block ID, edit timestamp or caption. Stop dependent implementation.', flush=True)
        return 1
    print('Image replacement changed proposed evidence; review timestamps before accepting.', flush=True)
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config', type=Path, required=True)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--prepare-images', metavar='TEST_CONTAINER_ID')
    mode.add_argument('--observe-images', metavar='TEST_CONTAINER_ID')
    mode.add_argument('--probe-rapid-image-edit', metavar='TEST_CONTAINER_ID')
    mode.add_argument('--qualify-image-bytes', metavar='TEST_CONTAINER_ID')
    args = parser.parse_args()
    if os.environ.get('DIARY_NOTION_LIVE') != '1':
        parser.exit(2, 'skipped: DIARY_NOTION_LIVE=1 is required\n')
    probe = Probe(private_token(args.config))
    print('API version:', VERSION, flush=True)
    if args.prepare_images or args.observe_images or args.probe_rapid_image_edit or args.qualify_image_bytes:
        parent = require_test_container(probe, args.prepare_images or args.observe_images or args.probe_rapid_image_edit or args.qualify_image_bytes)
        if args.qualify_image_bytes:
            qualify_image_bytes(probe, parent)
            return 0
        if args.probe_rapid_image_edit:
            return rapid_image_edit(probe, parent)
        if args.prepare_images:
            prepare_images(probe, parent)
        else:
            observe_images(probe, parent)
        return 0
    status, _ = probe.request('GET', 'pages/' + ROOT)
    print('authorized root read:', status, flush=True)
    if status != 200:
        return 1
    status, container = probe.create(ROOT, 'Diary upload qualification ' + datetime.now(timezone.utc).strftime('%Y-%m-%d %H:%M:%S UTC'))
    print('test container create:', status, flush=True)
    if status != 200:
        return 1
    parent = container['id']
    print('retained test container:', parent, flush=True)
    cases = [('2040-02', {'type': 'page_end'}), ('2040-01', {'type': 'page_start'}),
             ('2040-04', {'type': 'page_end'}), ('2040-03', None)]
    ids = {}
    for title, position in cases:
        if position is None:
            position = {'type': 'after_block', 'after_block': {'id': ids['2040-02']}}
        status, reply = probe.create(parent, title, position)
        print('position:', json.dumps(position), 'HTTP:', status,
              'code:', reply.get('code', 'ok'), flush=True)
        if status != 200:
            print('FAIL page positioning; stop dependent implementation. No write was retried.', flush=True)
            return 1
        ids[title] = reply['id']
        print('retained child:', title, reply['id'], flush=True)
    found = [b['id'] for b in probe.children(parent) if b['type'] == 'child_page']
    if found != [ids[k] for k in sorted(ids)]:
        print('FAIL page order read-back', flush=True)
        return 1
    print('PASS page positions and stable identities; image qualification remains required.', flush=True)
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError):
        # Exception details can contain private filesystem paths or response data.
        sys.exit('FAIL qualification transport/config/read error; writes may be uncertain; do not replay blindly')
