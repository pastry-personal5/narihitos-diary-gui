"""P1-M15 synthetic native Trash process faults + pinned producer catalog/reopen.

Runs outside the gate on macOS. Only freshly generated fixture members may move.
Cleans only native-returned receipts whose full original identity/digest still match.
Never enumerates Trash or reads a personal configuration. SKIP is not acceptance evidence.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import uuid

ROOT = Path(__file__).resolve().parents[2]
MOBILE = ROOT / 'contrib/narihitos-diary-mobile/NarihitosDiary'
FILES = ['Domain/DiarySnippet.swift', 'Application/AssetCatalog.swift',
         'Application/DiaryTitlePolicy.swift', 'Adapters/DiarySnippetLayout.swift',
         'Adapters/DiarySnippetMarkdownCodec.swift', 'Adapters/DiaryFileIdentity.swift',
         'Adapters/AppleDiaryFileObservation.swift', 'Adapters/BoundedFileCoordinator.swift',
         'Adapters/LegacyDiaryReader.swift', 'Adapters/CatalogSafeFiles.swift',
         'Adapters/CatalogAssetRegistry.swift', 'Adapters/CatalogAudioInspector.swift',
         'Adapters/DiaryCatalogHandler.swift', 'Adapters/DiaryCatalogCurrentReader.swift',
         'Adapters/DiaryCatalogLegacyReader.swift', 'Adapters/DiaryCatalogReadPlan.swift',
         'Adapters/FileCatalogWorker.swift']
WORKER = 'source::trash::service::native_harness::native_process_worker'
if sys.platform != 'darwin' or not shutil.which('swiftc') or not all((MOBILE / f).exists() for f in FILES):
    print('SKIP source Trash: macOS, swiftc and pinned producer required (not evidence)')
    sys.exit(0)


def run(command, **kwargs):
    result = subprocess.run(command, capture_output=True, text=True, **kwargs)
    if result.returncode:
        raise RuntimeError(f'command failed ({result.returncode}): {result.stderr[-4000:]} {result.stdout[-2000:]}')
    return result.stdout


def document(ident, body):
    fields = [('schema_version', '1'), ('kind', '"ndm.diary-snippet"'), ('id', json.dumps(ident)),
              ('created_at', '"2024-02-29T12:00:00Z"'), ('updated_at', '"2024-02-29T12:00:00Z"'),
              ('title', '"Hello"'), ('title_origin', '"first-sentence"'), ('favorite', 'false'),
              ('language', '"en"'), ('audio_filename', json.dumps(ident + '.m4a')),
              ('recording_duration_ms', '1000'), ('audio_codec', '"aac"'), ('audio_sample_rate_hz', '44100'),
              ('audio_channels', '1'), ('audio_bitrate_bps', '64000'),
              ('transcription_method_id', '"ndm.transcription.apple"'), ('transcription_language_hint', '"en"'),
              ('transcription_settings_revision', '1')]
    return ('---\n' + '\n'.join(f'{k}: {v}' for k, v in fields) + '\n---\n\n' + body).encode()


def member(source, ident, body, audio):
    relative = f'2024/02/29/2024-02-29_12-00-00-{ident}.md'
    md = source / relative
    md.parent.mkdir(parents=True, exist_ok=True)
    md.write_bytes(document(ident, body))
    assets = md.with_suffix('.assets')
    assets.mkdir()
    recording = assets / (ident + '.m4a')
    if audio:
        shutil.copyfile(audio, recording)
    return relative, md, recording, assets


with tempfile.TemporaryDirectory(prefix='diary-trash-build-') as build_temp:
    build = Path(build_temp)
    producer = build / 'producer'
    # The worker's unused editing entry point names this plain capability type.
    # Extract the exact pinned declaration; no catalog/read/recovery logic is replaced.
    action_source = MOBILE / 'Adapters/CommittedDiaryResource.swift'
    declaration = action_source.read_text().split('struct CatalogActionResource: Sendable {', 1)[1].split('\n}', 1)[0]
    support = build / 'CatalogActionResource.swift'
    support.write_text('import Foundation\nstruct CatalogActionResource: Sendable {' + declaration + '\n}\n')
    run(['swiftc', '-swift-version', '5', '-parse-as-library', '-module-cache-path', str(build / 'modules'),
         '-o', str(producer), str(ROOT / 'scripts/smoke/trash/main.swift'), str(support), *[str(MOBILE / f) for f in FILES]])
    audio = build / 'valid.m4a'
    run([str(producer), 'audio', str(audio)])
    target = ROOT / 'src-tauri/target/trash-harness'
    output = run(['cargo', 'test', '--quiet', '--manifest-path', str(ROOT / 'src-tauri/Cargo.toml'),
                  '-p', 'diary-infrastructure', '--features', 'fault-injection', '--target-dir', str(target),
                  '--no-run', '--message-format=json'])
    binaries = [json.loads(line)['executable'] for line in output.splitlines()
                if line.startswith('{') and json.loads(line).get('reason') == 'compiler-artifact'
                and json.loads(line).get('executable') and json.loads(line).get('profile', {}).get('test')]
    assert len(binaries) == 1
    worker = binaries[0]
    evidence = []
    scenarios = [
        ('full', None, 'succeeded', ['moved', 'moved']),
        ('missing', None, 'succeeded', ['moved', 'missing']),
        ('markdown-failure', 'trash:native_failure:0', 'needs_confirmation', ['failed', 'pending']),
        ('audio-failure', 'trash:native_failure:1', 'needs_confirmation', ['moved', 'failed']),
        ('after-admission', 'trash:after_admission:kill', 'needs_confirmation', ['failed', 'pending']),
        ('before-intent', 'trash:before_intent:0:kill', 'needs_confirmation', ['failed', 'pending']),
        ('after-intent', 'trash:after_intent:0:kill', 'uncertain', ['uncertain', 'pending']),
        ('before-native', 'trash:before_native:0:kill', 'uncertain', ['uncertain', 'pending']),
        ('before-receipt-markdown', 'trash:before_receipt:0:kill', 'uncertain', ['uncertain', 'pending']),
        ('before-intent-audio', 'trash:before_intent:1:kill', 'needs_confirmation', ['moved', 'failed']),
        ('after-intent-audio', 'trash:after_intent:1:kill', 'uncertain', ['moved', 'uncertain']),
        ('before-native-audio', 'trash:before_native:1:kill', 'uncertain', ['moved', 'uncertain']),
        ('before-receipt-audio', 'trash:before_receipt:1:kill', 'uncertain', ['moved', 'uncertain']),
        ('after-native-markdown', 'trash:after_native:0:kill', 'uncertain', ['uncertain', 'pending']),
        ('after-receipt-markdown', 'trash:after_receipt:0:kill', 'needs_confirmation', ['moved', 'failed']),
        ('after-native-audio', 'trash:after_native:1:kill', 'uncertain', ['moved', 'uncertain']),
        ('after-receipt-audio', 'trash:after_receipt:1:kill', 'succeeded', ['moved', 'moved']),
        ('receipt-db-error', 'database-receipt-failure', 'uncertain', ['uncertain', 'pending']),
        ('receipt-error', 'trash:before_receipt:0:io', 'uncertain', ['uncertain', 'pending']),
        ('replacement-markdown', 'trash:native_failure:1', 'needs_confirmation', ['moved', 'failed']),
        ('replacement-audio', 'trash:native_failure:1', 'needs_confirmation', ['moved', 'failed']),
    ]
    for name, fault, expected_state, states in scenarios:
        with tempfile.TemporaryDirectory(prefix='diary-trash-smoke-') as temporary:
            base = Path(temporary).resolve()
            base.chmod(0o700)
            (base / 'fixture-marker').write_text('synthetic-source-trash-v1')
            source = base / 'source'
            source.mkdir(mode=0o700)
            ident, neighbor_id = str(uuid.uuid4()), str(uuid.uuid4())
            relative, md, recording, assets = member(source, ident, 'target\n', None if name == 'missing' else audio)
            _, neighbor, neighbor_audio, neighbor_assets = member(source, neighbor_id, 'neighbor\n', audio)
            (base / 'target-relative').write_text(relative)
            sentinel = source / 'catalog-sentinel'
            sentinel.write_bytes(b'not a producer catalog; preserve exactly')
            hashes = {p: hashlib.sha256(p.read_bytes()).digest() for p in [neighbor, neighbor_audio, sentinel]}

            def invoke(action, injected=None):
                env = dict(os.environ, DIARY_TRASH_FIXTURE=str(base), DIARY_TRASH_ACTION=action)
                env.pop('DIARY_FAULT', None)
                if injected == 'database-receipt-failure':
                    env['DIARY_HARNESS_DB_FAIL'] = '1'
                elif injected:
                    env['DIARY_FAULT'] = injected
                result = subprocess.run([worker, WORKER, '--exact', '--ignored'], env=env, capture_output=True, text=True)
                if injected and injected.endswith(':kill'):
                    assert result.returncode == 86, result.stderr + result.stdout
                    return None
                assert result.returncode == 0, result.stderr + result.stdout
                if action != 'cleanup':
                    return json.loads((base / 'reply.json').read_text())

            def catalog():
                result = json.loads(run([str(producer), 'catalog', str(source)]))
                assert any(s['asset_id'] == neighbor_id and s['body'] == 'neighbor\n' and s['audio_available'] for s in result['summaries']), result
                for p, digest in hashes.items():
                    assert p.is_file() and hashlib.sha256(p.read_bytes()).digest() == digest
                assert assets.is_dir() and neighbor_assets.is_dir()
                return result

            def catalog_state(result):
                target_present = md.exists() and recording.exists()
                expected_summaries = 2 if target_present else 1
                assert len(result['summaries']) == expected_summaries, (name, result)
                if target_present:
                    assert result['recovery'] == [], (name, result)
                elif not md.exists() and recording.exists():
                    assert result['recovery'] == [{'issue': 'missingMarkdown', 'has_text': False, 'has_audio': True}], (name, result)
                elif not md.exists() and not recording.exists():
                    # The producer retains a recovery candidate from the empty .assets directory.
                    assert result['recovery'] == [{'issue': 'changed', 'has_text': False, 'has_audio': False}], (name, result)
                else:
                    assert result['recovery'] == [{'issue': 'missingAudio', 'has_text': True, 'has_audio': False}], (name, result)

            before = catalog()
            catalog_state(before)
            assert len(before['summaries']) == (1 if name == 'missing' else 2), before
            try:
                initial = invoke('initial', fault)
                result = invoke('recover')
                assert result['state'] == expected_state, (name, result)
                assert [m['state'] for m in result['members']] == states, (name, result)
                after = catalog()
                catalog_state(after)
                duplicate = invoke('duplicate')
                assert duplicate['ack'] == 1 and duplicate['members'] == result['members'], (name, duplicate)
                retried = None
                catalog_retry = None
                if name.startswith('replacement-'):
                    if name == 'replacement-markdown':
                        md.write_bytes(document(ident, 'replacement\n'))
                        replacement = md
                    else:
                        recording.unlink()
                        recording.write_bytes(b'replacement-audio-preserve')
                        replacement = recording
                    digest = hashlib.sha256(replacement.read_bytes()).digest()
                    retried = invoke('retry')
                    assert 'error' in retried and hashlib.sha256(replacement.read_bytes()).digest() == digest
                    catalog_retry = catalog()
                elif expected_state == 'needs_confirmation':
                    retried = invoke('retry')
                    assert retried['state'] == 'succeeded' and retried['ack'] == 2, (name, retried)
                    catalog_retry = catalog()
                elif expected_state == 'uncertain':
                    retried = invoke('retry')
                    assert retried['error'] == 'Uncertain', (name, retried)
                evidence.append({'scenario': name, 'fault': fault, 'result': result,
                                 'catalog_before': before, 'catalog_after': after, 'retry': retried, 'catalog_retry': catalog_retry})
                print(f'PASS {name}', flush=True)
            finally:
                invoke('cleanup')
    if '--enabled-server' in sys.argv:
        sys.path.insert(0, str(ROOT / 'scripts/smoke/trash'))
        from server import verify_server
        evidence.append(verify_server(ROOT, producer, worker, audio, member))
    record = {'producer_commit': run(['git', '-C', str(MOBILE.parent), 'rev-parse', 'HEAD']).strip(),
              'producer_files': FILES, 'unchanged_declaration': 'Adapters/CommittedDiaryResource.swift: CatalogActionResource', 'scenarios': evidence, 'audio': 'AVFoundation generated AAC/M4A; actual producer AudioToolbox validation',
              'limits': 'Synthetic local macOS files; no phone app/live iCloud claim; native failures are injected returned errors.'}
    report = ROOT / 'src-tauri/target/trash-harness-evidence.json'
    report.write_text(json.dumps(record, indent=2) + '\n')
    print('PASS native source Trash and producer catalog/reopen matrix; report: src-tauri/target/trash-harness-evidence.json')
