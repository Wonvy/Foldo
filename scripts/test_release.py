import json
import pathlib
import tempfile
import unittest
import release


class ReleaseTests(unittest.TestCase):
    def test_complete_manifest_and_missing_artifacts(self):
        tag = 'v' + release.version()
        with tempfile.TemporaryDirectory() as temp:
            folder = pathlib.Path(temp)
            suffixes = [
                'desktop-' + tag + '-windows-x64-setup.exe',
                'desktop-' + tag + '-macos-arm64.app.tar.gz',
                'desktop-' + tag + '-macos-arm64.dmg',
                'desktop-' + tag + '-linux-x64.AppImage',
                'desktop-' + tag + '-linux-x64.deb',
                tag + '-windows-x64.zip', tag + '-macos-arm64.tar.gz',
                tag + '-macos-x64.tar.gz', tag + '-linux-x64.tar.gz',
            ]
            for suffix in suffixes:
                (folder / ('foldo-' + suffix)).write_bytes(b'fixture')
            signed = [suffixes[0], suffixes[1], suffixes[3]]
            for suffix in signed:
                (folder / ('foldo-' + suffix + '.sig')).write_text('test-signature')
            release.manifest(folder, tag, 'Wonvy/Foldo')
            data = json.loads((folder / 'latest.json').read_text())
            self.assertEqual(set(data['platforms']), {'windows-x86_64', 'darwin-aarch64', 'linux-x86_64'})
            self.assertEqual(data['version'], release.version())
            for value in data['platforms'].values():
                self.assertTrue((folder / value['url'].rsplit('/', 1)[1]).is_file())
            self.assertIn('latest.json', (folder / 'SHA256SUMS.txt').read_text())
            signature = folder / ('foldo-' + signed[0] + '.sig')
            signature.unlink()
            with self.assertRaisesRegex(AssertionError, 'Missing signature'):
                release.manifest(folder, tag, 'Wonvy/Foldo')
            signature.write_text('test-signature')
            (folder / ('foldo-' + suffixes[2])).unlink()
            with self.assertRaisesRegex(AssertionError, 'Missing asset'):
                release.manifest(folder, tag, 'Wonvy/Foldo')

    def test_tag_must_match_app(self):
        with self.assertRaisesRegex(AssertionError, 'Tag must match'):
            release.manifest(pathlib.Path('.'), 'v999.0.0', 'Wonvy/Foldo')
