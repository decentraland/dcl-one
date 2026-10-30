#!/usr/bin/env python3
"""Install the pinned protoc compiler on the Explorer CI runners."""
import hashlib
import io
import os
from pathlib import Path
import platform
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import zipfile

VERSION = '26.1'
# Official v26.1 release archives; choose the runner host, including cross builds.
ARCHIVES = {
    ('Linux', 'x86_64'): ('linux-x86_64', 'a7be2928c0454f132c599e25b79b7ad1b57663f2337d7f7e468a1d59b98ec1b0'),
    ('Darwin', 'arm64'): ('osx-aarch_64', '26a29befa8891ecc48809958c909d284f2b9539a2eb47f22cadc631fe6abe8fd'),
    ('Windows', 'amd64'): ('win64', '9090d135a1159042b13b4e51b210e40cb820d85a5032a6eca5f9b3ca3bdfb539'),
}


def download(url):
    for attempt in range(3):
        try:
            with urllib.request.urlopen(url, timeout=30) as response:
                return response.read()
        except (urllib.error.URLError, TimeoutError):
            if attempt == 2:
                raise
            time.sleep(2 ** attempt)


def install(system, machine, destination):
    archive, digest = ARCHIVES[(system, machine.lower())]
    url = f'https://github.com/protocolbuffers/protobuf/releases/download/v{VERSION}/protoc-{VERSION}-{archive}.zip'
    data = download(url)
    if hashlib.sha256(data).hexdigest() != digest:
        raise ValueError('protoc archive SHA-256 does not match the pin')
    with zipfile.ZipFile(io.BytesIO(data)) as bundle:
        bundle.extractall(destination)
    executable = destination / 'bin' / ('protoc.exe' if system == 'Windows' else 'protoc')
    executable.chmod(0o755)
    actual = subprocess.check_output([str(executable), '--version'], text=True).strip()
    if actual != f'libprotoc {VERSION}':
        raise ValueError(f'Unexpected compiler version: {actual}')
    print(actual)
    return executable.parent


def main():
    destination = Path(tempfile.mkdtemp(prefix=f'bevy-protoc-{VERSION}-', dir=os.environ['RUNNER_TEMP']))
    binary_dir = install(platform.system(), platform.machine(), destination)
    with open(os.environ['GITHUB_PATH'], 'a', encoding='utf-8') as path:
        path.write(f'{binary_dir}\n')


if __name__ == '__main__':
    main()
