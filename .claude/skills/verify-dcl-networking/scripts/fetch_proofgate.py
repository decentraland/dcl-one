"""Fetch the reviewed proof gate without installing or changing global config."""
import argparse
import hashlib
from pathlib import Path
import urllib.request

PIN = '650e43dc362f9fc0c6601c8537b42aa2255d5db7'
SHA256 = '420f0b8fcf644add2dfcf983909ca99d3af9b26824bfe87dc116bf8a88371676'
URL = f'https://raw.githubusercontent.com/eordano/rocq-verify/{PIN}/proofgate.py'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dest', required=True, type=Path, help='Absolute task tools directory')
    parser.add_argument('--source', type=Path, help='Offline copy; must match the pinned hash')
    args = parser.parse_args()
    if not args.dest.is_absolute():
        parser.error('--dest must be absolute')
    output = args.dest / 'proofgate.py'
    if output.exists():
        if hashlib.sha256(output.read_bytes()).hexdigest() != SHA256:
            raise SystemExit('Refusing to overwrite a different existing proofgate.py')
        print(f'Already verified {output}')
        return
    if args.source:
        data = args.source.read_bytes()
    else:
        with urllib.request.urlopen(URL, timeout=30) as response:
            data = response.read()
    if hashlib.sha256(data).hexdigest() != SHA256:
        raise SystemExit('Pinned proofgate.py SHA-256 mismatch; no file written')
    args.dest.mkdir(parents=True, exist_ok=True)
    with output.open('xb') as stream:
        stream.write(data)
    print(f'Verified {output} at {PIN}')


if __name__ == '__main__':
    main()
