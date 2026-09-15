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

"""Verify promoted ASF source files before GitHub or registry publication."""

import argparse
import hashlib
import io
from pathlib import Path, PurePosixPath
import re
import subprocess
import tarfile
import tempfile
import urllib.request
import tomllib
import zipfile

ASF = 'https://downloads.apache.org/incubator/casbin/'


def source_files(data, extension):
    files = {}
    def safe_name(name):
        if '\\' in name or PurePosixPath(name).is_absolute() or '..' in PurePosixPath(name).parts:
            raise ValueError('Unsafe archive path')
    if extension == 'zip':
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            for item in archive.infolist():
                safe_name(item.filename)
                if (item.external_attr >> 16) & 0o170000 == 0o120000:
                    raise ValueError('ZIP symlink is unsupported')
                if not item.is_dir():
                    if item.filename in files:
                        raise ValueError('Duplicate ZIP member')
                    files[item.filename] = archive.read(item)
    else:
        with tarfile.open(fileobj=io.BytesIO(data), mode='r:*') as archive:
            for item in archive:
                safe_name(item.name)
                if item.isdir():
                    continue
                if not item.isfile() or item.name in files:
                    raise ValueError('Unsupported or duplicate TAR member')
                files[item.name] = archive.extractfile(item).read()
    return files


def check_checksum(data, checksum, filename):
    match = re.fullmatch(r'([0-9a-fA-F]{128}) [ *]' + re.escape(filename) + r'\r?\n?', checksum.decode('ascii'))
    if not match or hashlib.sha512(data).hexdigest() != match[1].lower():
        raise ValueError('Invalid SHA-512 for ' + filename)


def download(url):
    with urllib.request.urlopen(url, timeout=60) as response:
        return response.read()


def verify_signature(home, signature, archive):
    subprocess.run(['gpg', '--batch', '--homedir', str(home), '--verify', str(signature), str(archive)], check=True)


def manifest_version():
    with open('Cargo.toml', 'rb') as source:
        return tomllib.load(source)['package']['version']


def verify(version, output):
    if not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+', version):
        raise ValueError('Expected final version X.Y.Z')
    pom_version = manifest_version()
    if pom_version != version:
        raise ValueError('Cargo manifest does not match requested version')
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
    name = 'apache-casbin-sqlx-adapter-' + version + '-incubating-src'
    expected = source_files(subprocess.check_output(['git', 'archive', '--format=tar', '--prefix=' + name + '/', commit]), 'tar')
    output.mkdir(parents=True, exist_ok=False)
    url = ASF + 'sqlx-adapter-' + version + '-incubating/'
    with tempfile.TemporaryDirectory(prefix='sqlx-adapter-public-keys-') as temp:
        home = Path(temp)
        home.chmod(0o700)
        keys = home / 'KEYS'
        keys.write_bytes(download(ASF + 'KEYS'))
        subprocess.run(['gpg', '--batch', '--homedir', str(home), '--import', str(keys)], check=True)
        for extension in ('tar.gz',):
            filename = name + '.' + extension
            archive = output / filename
            for suffix in ('', '.sha512', '.asc'):
                (output / (filename + suffix)).write_bytes(download(url + filename + suffix))
            data = archive.read_bytes()
            check_checksum(data, (output / (filename + '.sha512')).read_bytes(), filename)
            verify_signature(home, output / (filename + '.asc'), archive)
            if source_files(data, extension) != expected:
                raise ValueError('Promoted archive does not match final Git commit: ' + filename)
    print('Verified promoted ASF originals for ' + commit)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('version')
    parser.add_argument('--output', type=Path, default=Path('dist'))
    args = parser.parse_args()
    verify(args.version, args.output)
