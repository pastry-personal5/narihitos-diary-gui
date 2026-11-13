"""Offline safety checks for the opt-in service qualification probe."""
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    'notion_qualification', Path(__file__).resolve().parents[1] / 'smoke/notion-page-qualification.py')
live = importlib.util.module_from_spec(spec)
spec.loader.exec_module(live)


class QualificationSafety(unittest.TestCase):
    def test_token_reader_ignores_roots_and_rejects_symlinks_and_public_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            config = Path(temporary) / 'config.yaml'
            config.write_text('notion:\n  root_page_id: production-root\n  token: "ntn_fixture"\n')
            config.chmod(0o600)
            self.assertEqual(live.private_token(config), 'ntn_fixture')
            link = Path(temporary) / 'link.yaml'
            link.symlink_to(config)
            with self.assertRaises(ValueError):
                live.private_token(link)
            config.chmod(0o644)
            with self.assertRaises(ValueError):
                live.private_token(config)

    def test_token_reader_rejects_duplicate_and_nonliteral_tokens(self):
        with tempfile.TemporaryDirectory() as temporary:
            config = Path(temporary) / 'config.yaml'
            for body in ['notion:\n  token: a\n  token: b\n',
                         'notion:\n  token: !secret token\n',
                         'notion:\n  token: a\nnotion:\n  token: b\n']:
                config.write_text(body)
                config.chmod(0o600)
                with self.assertRaises(ValueError):
                    live.private_token(config)

    def test_live_gate_precedes_config_and_network_access(self):
        with patch.dict(os.environ, {'DIARY_NOTION_LIVE': '0'}), \
                patch('sys.argv', ['probe', '--config', '/not/read']), \
                patch.object(live, 'private_token') as token, \
                patch.object(live, 'Probe') as probe:
            with self.assertRaises(SystemExit) as error:
                live.main()
            self.assertEqual(error.exception.code, 2)
            token.assert_not_called()
            probe.assert_not_called()

    def test_image_modes_reject_production_parent_before_write(self):
        probe = live.Probe('fixture')
        with patch.object(probe, 'request', return_value=(200, {
                'parent': {'page_id': '00000000-0000-0000-0000-000000000001'}})) as request:
            with self.assertRaises(ValueError):
                live.require_test_container(probe, '00000000-0000-0000-0000-000000000002')
            request.assert_called_once_with('GET', 'pages/00000000-0000-0000-0000-000000000002')

    def test_observations_never_print_signed_urls_or_caption_text(self):
        probe = live.Probe('private-token')
        block = {'id': 'fixture', 'type': 'image', 'last_edited_time': 'timestamp', 'image': {
            'type': 'file', 'file': {'url': 'https://host/private-path?secret=signature',
                                    'expiry_time': 'expiry'},
            'caption': [{'plain_text': 'private-caption'}]}}
        with patch.object(probe, 'children', return_value=[block]), \
                patch('sys.stdout', new_callable=io.StringIO) as output:
            live.observe_images(probe, 'parent')
        observed = output.getvalue()
        for private in ['private-token', 'private-path', 'signature', 'private-caption']:
            self.assertNotIn(private, observed)
        self.assertEqual(json.loads(observed)['image_observation'][0]['file_keys'], ['expiry_time', 'url'])

    def test_position_rejection_stops_without_retry_or_more_writes(self):
        probe = live.Probe('fixture')
        with patch.dict(os.environ, {'DIARY_NOTION_LIVE': '1'}), \
                patch('sys.argv', ['probe', '--config', 'unused']), \
                patch.object(live, 'private_token', return_value='fixture'), \
                patch.object(live, 'Probe', return_value=probe), \
                patch.object(probe, 'request', return_value=(200, {})), \
                patch.object(probe, 'create', side_effect=[
                    (200, {'id': 'test-container'}), (400, {'code': 'validation_error'})]) as create, \
                patch('sys.stdout', new_callable=io.StringIO):
            self.assertEqual(live.main(), 1)
            self.assertEqual(create.call_count, 2)
            self.assertEqual(create.call_args_list[0].args[0], live.ROOT)

    def test_same_minute_image_replacement_fails_proposed_drift_evidence(self):
        block_id = '00000000-0000-0000-0000-000000000002'
        before = {'id': block_id, 'type': 'image', 'last_edited_time': '2026-11-13T10:00:00.000Z',
                  'image': {'type': 'file', 'caption': [], 'file': {'url': 'https://host/blue.jpg'}}}
        after = {**before, 'image': {**before['image'], 'file': {'url': 'https://host/red.jpg'}}}
        probe = live.Probe('fixture')
        with patch.object(live, 'fixture_jpeg', return_value=b'jpeg'), \
                patch.object(live, 'upload_fixture', side_effect=['blue', 'red']), \
                patch.object(probe, 'request', side_effect=[
                    (200, {'results': [{'id': block_id}]}), (200, before),
                    (200, after), (200, after), (200, {'status': 'uploaded'})]) as request, \
                patch('sys.stdout', new_callable=io.StringIO) as output:
            self.assertEqual(live.rapid_image_edit(probe, 'parent'), 1)
        self.assertIn('FAIL drift evidence', output.getvalue())
        self.assertEqual(request.call_count, 5)
        self.assertEqual(request.call_args_list[2].args,
                         ('PATCH', 'blocks/' + block_id, {'image': {'file_upload': {'id': 'red'}}}))

    def test_image_download_rejects_foreign_hosts_credentials_and_cleartext(self):
        from unittest.mock import Mock
        opener = Mock()
        for url in ['http://file.notion.so/file', 'https://evil.example/file',
                    'https://file.notion.so.evil.example/file',
                    'https://user:password@file.notion.so/file',
                    'https://file.notion.so:8443/file']:
            with self.assertRaises(ValueError):
                live.download_image_hash(opener, url)
        opener.open.assert_not_called()

    def test_image_download_is_bounded_and_sends_no_integration_credential(self):
        from unittest.mock import MagicMock
        opener, response = MagicMock(), MagicMock()
        opener.open.return_value.__enter__.return_value = response
        response.headers = {}
        response.read.return_value = b'fixture-jpeg'
        result = live.download_image_hash(opener, 'https://file.notion.so/fixture?signature=private')
        self.assertEqual(result['bytes'], 12)
        request = opener.open.call_args.args[0]
        self.assertNotIn('Authorization', request.headers)
        response.read.assert_called_once_with(20 * 1024 * 1024 + 1)
        response.headers = {'Content-Length': str(20 * 1024 * 1024 + 1)}
        with self.assertRaises(ValueError):
            live.download_image_hash(opener, 'https://file.notion.so/fixture')


if __name__ == '__main__':
    unittest.main()
