# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements. See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership. The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License. You may obtain a copy of the License at
#
#     https://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied. See the License for the
# specific language governing permissions and limitations
# under the License.

import hashlib
import io
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import verify_release as release


class ReleaseVerificationTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.previous = Path.cwd()
        self.repo = Path(self.temp.name)
        os.chdir(self.repo)
        self.addCleanup(os.chdir, self.previous)
        self.git('init', '-q')
        Path('Cargo.toml').write_text('[package]\nname = \"sqlx-adapter\"\nversion = \"1.0.0\"\n')
        Path('LICENSE').write_text('Test fixture')
        self.git('add', '.')
        self.git('-c', 'user.name=Release test', '-c', 'user.email=release@example.invalid', 'commit', '-qm', 'test: fixture')
        self.name = 'apache-casbin-sqlx-adapter-1.0.0-incubating-src'
        self.remote = {release.ASF + 'KEYS': b'test public key'}
        for extension in ('tar.gz',):
            data = self.git('archive', '--format=' + extension, '--prefix=' + self.name + '/', 'HEAD')
            self.put_archive(extension, data)

    def git(self, *args):
        return subprocess.check_output(['git', *args], stderr=subprocess.PIPE)

    def put_archive(self, extension, data):
        filename = self.name + '.' + extension
        url = release.ASF + 'sqlx-adapter-1.0.0-incubating/' + filename
        self.remote[url] = data
        self.remote[url + '.sha512'] = (hashlib.sha512(data).hexdigest() + '  ' + filename + '\n').encode()
        self.remote[url + '.asc'] = b'test signature'

    def invoke(self, signature_error=None):
        # No live downloads or signing material: exercise real Git/archive/hash
        # verification and inject only the external network/GPG boundaries.
        real_run = subprocess.run
        def run_command(command, **kwargs):
            if command[0] == 'gpg':
                return subprocess.CompletedProcess(command, 0)
            return real_run(command, **kwargs)
        with patch.object(release, 'download', side_effect=self.remote.__getitem__), \
             patch.object(release.subprocess, 'run', side_effect=run_command) as calls, \
             patch.object(release, 'verify_signature', side_effect=signature_error) as signature:
            release.verify('1.0.0', Path('dist'))
        self.assertEqual(sum(call.args[0][0] == 'gpg' for call in calls.call_args_list), 1)
        self.assertEqual(signature.call_count, 1)

    def test_preserves_all_three_promoted_files(self):
        self.invoke()
        files = list(Path('dist').iterdir())
        self.assertEqual(len(files), 3)
        for file in files:
            self.assertEqual(file.read_bytes(), self.remote[release.ASF + 'sqlx-adapter-1.0.0-incubating/' + file.name])

    def test_rejects_missing_signature(self):
        self.remote.pop(release.ASF + 'sqlx-adapter-1.0.0-incubating/' + self.name + '.tar.gz.asc')
        with self.assertRaises(KeyError):
            self.invoke()

    def test_rejects_signature_failure(self):
        with self.assertRaises(subprocess.CalledProcessError):
            self.invoke(subprocess.CalledProcessError(1, 'gpg'))

    def test_rejects_corrupt_archive(self):
        self.remote[release.ASF + 'sqlx-adapter-1.0.0-incubating/' + self.name + '.tar.gz'] += b'corrupt'
        with self.assertRaises(ValueError):
            self.invoke()

    def test_rejects_different_source_with_valid_checksum(self):
        Path('LICENSE').write_text('Changed after vote')
        self.git('add', 'LICENSE')
        self.git('-c', 'user.name=Release test', '-c', 'user.email=release@example.invalid', 'commit', '-qm', 'test: changed tree')
        with self.assertRaises(ValueError):
            self.invoke()

    def test_rejects_missing_file_with_valid_checksum(self):
        data = self.git('archive', '--format=tar.gz', '--prefix=' + self.name + '/', 'HEAD', 'LICENSE')
        self.put_archive('tar.gz', data)
        with self.assertRaises(ValueError):
            self.invoke()

    def test_metadata_validates_version_and_rc_before_publication(self):
        from release_meta import metadata
        self.assertEqual(metadata('v1.0.0')['is_rc'], 'false')
        self.assertEqual(metadata('v1.0.0-rc1')['is_rc'], 'true')
        self.assertEqual(metadata('v1.0.0')['basename'], metadata('v1.0.0-rc1')['basename'])
        for tag in ['v1.0.1', 'v1.0.0-rc0', 'v1.0.0-snapshot.1', '1.0.0', 'master']:
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                metadata(tag)

    def test_rejects_overwriting_existing_output(self):
        Path('dist').mkdir()
        Path('dist/sentinel').write_text('keep')
        with self.assertRaises(FileExistsError):
            self.invoke()
        self.assertEqual(Path('dist/sentinel').read_text(), 'keep')

    def test_rejects_invalid_version_before_network(self):
        for version in ('1.0.0-rc1', '../1.0.0', '1.0.1'):
            with self.subTest(version=version), patch.object(release, 'download') as network:
                with self.assertRaises(ValueError):
                    release.verify(version, Path('dist'))
                network.assert_not_called()

    def test_rejects_bad_checksum_filename(self):
        with self.assertRaises(ValueError):
            release.check_checksum(b'x', (hashlib.sha512(b'x').hexdigest() + '  other\n').encode(), 'expected')

    def test_rejects_archive_traversal_and_links(self):
        for name, kind in (('../escape', tarfile.REGTYPE), ('root/link', tarfile.SYMTYPE)):
            buf = io.BytesIO()
            with tarfile.open(fileobj=buf, mode='w') as archive:
                item = tarfile.TarInfo(name)
                item.type = kind
                archive.addfile(item)
            with self.assertRaises(ValueError):
                release.source_files(buf.getvalue(), 'tar')

    def test_gpg_verification_is_checked_and_uses_isolated_home(self):
        with patch.object(release.subprocess, 'run') as gpg:
            release.verify_signature(Path('isolated'), Path('source.asc'), Path('source'))
        gpg.assert_called_once_with(['gpg', '--batch', '--homedir', 'isolated', '--verify', 'source.asc', 'source'], check=True)


if __name__ == '__main__':
    unittest.main()
