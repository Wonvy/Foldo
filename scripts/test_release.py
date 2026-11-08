import json
import copy
import hashlib
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

    def uploaded_fixture(self):
        tag = 'v' + release.version()
        metadata = {
            'version': release.version(), 'pub_date': '2026-11-08T00:00:00Z',
            'platforms': {platform: {'signature': 'signed-' + platform, 'url': 'old-url'}
                          for platform in release.updater_assets(tag)},
        }
        assets = [
            {'id': index, 'name': name, 'state': 'uploaded', 'size': 42,
             'browser_download_url': f'https://github.com/Wonvy/Foldo/releases/download/{tag}/{name}'}
            for index, name in enumerate(release.updater_assets(tag).values(), 1)
        ]
        return tag, metadata, assets

    def test_asset_api_urls_preserve_signatures_and_refresh_checksum(self):
        tag, metadata, assets = self.uploaded_fixture()
        with tempfile.TemporaryDirectory() as temp:
            folder = pathlib.Path(temp)
            manifest = folder / 'latest.json'
            manifest.write_text(json.dumps(metadata), encoding='utf-8')
            (folder / 'package.exe').write_bytes(b'original signed package')
            release.link_assets(folder, tag, 'Wonvy/Foldo', assets)
            updated = json.loads(manifest.read_text(encoding='utf-8'))
            self.assertEqual(updated['pub_date'], metadata['pub_date'])
            for index, platform in enumerate(release.updater_assets(tag), 1):
                self.assertEqual(updated['platforms'][platform]['signature'], metadata['platforms'][platform]['signature'])
                self.assertEqual(updated['platforms'][platform]['url'], f'https://api.github.com/repos/Wonvy/Foldo/releases/assets/{index}')
            checksums = (folder / 'SHA256SUMS.txt').read_text()
            self.assertIn(hashlib.sha256(manifest.read_bytes()).hexdigest() + '  latest.json', checksums)
            self.assertIn(hashlib.sha256(b'original signed package').hexdigest() + '  package.exe', checksums)

    def test_incomplete_or_wrong_uploaded_assets_are_rejected_without_changing_manifest(self):
        tag, metadata, assets = self.uploaded_fixture()
        invalid = [assets[:-1], assets + [assets[0]]]
        for key, value in [('id', 0), ('id', True), ('size', 0), ('state', 'starter'),
                           ('browser_download_url', 'https://github.com/other/repo/releases/download/v1/package.exe')]:
            altered = copy.deepcopy(assets)
            altered[0][key] = value
            invalid.append(altered)
        for uploaded in invalid:
            with self.subTest(uploaded=uploaded):
                original = copy.deepcopy(metadata)
                with self.assertRaises(AssertionError):
                    release.asset_download_urls(metadata, tag, 'Wonvy/Foldo', uploaded)
                self.assertEqual(metadata, original)

    def test_asset_manifest_version_must_match(self):
        _, metadata, assets = self.uploaded_fixture()
        with self.assertRaisesRegex(AssertionError, 'version must match'):
            release.asset_download_urls(metadata, 'v999.0.0', 'Wonvy/Foldo', assets)
