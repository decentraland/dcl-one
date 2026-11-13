"""Copies the blobs a source's imported rows name into the node's blob store (`migrate.py blobs`),
and the badge art the imported catalog names into the node's `BADGES_ASSETS_DIR`.

The store layout is `services/catalyst/storage`'s: `<STORAGE_ROOT_FOLDER>/contents/<shard>/<cid>`,
shard = the first four hex digits of sha1(cid), written through a `<name>.<pid>.<seq>.tmp` staging
file and a rename (the node's startup sweep removes a staging file a crash leaves behind). Every
object's bytes are re-hashed before they are stored; the CID functions reproduce
`library/dcl-auth/src/hash.rs`.
"""
import base64
import collections
import contextlib
import csv
import dataclasses
import errno
import gzip
import hashlib
import itertools
import os
import re
import stat
import sys
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

CHUNK_SIZE = 262_144
MAX_CHILDREN = 174
READ_SIZE = 1 << 20
CIDV0 = re.compile(r'Qm[1-9A-HJ-NP-Za-km-z]{44}')
CIDV1 = re.compile(r'ba[a-z2-7]{56,98}')
SHA256_HEX = re.compile(r'[0-9a-f]{64}')
# Upstream reads retried per key; a key still failing is reported, not recorded, and retried next run.
READ_ATTEMPTS = 3
RETRY_DELAY = 0.5
# Keys the preflight probe reads before a copy: new keys, plus keys an earlier run copied.
PROBE_KEYS = 16
PROBE_REFERENCE_KEYS = 4
# S3 answers worth retrying per key (throttling, server errors).
S3_RETRYABLE = {'SlowDown', 'InternalError', 'ServiceUnavailable', 'RequestTimeout', 'Throttling',
                'ThrottlingException'}
B58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
_seq = itertools.count()


# ── CIDs (library/dcl-auth/src/{hash,cid,unixfs}.rs) ────────────────────────

def is_canonical_cid(value):
    return bool(CIDV0.fullmatch(value) or CIDV1.fullmatch(value))


def _varint(n):
    out = bytearray()
    while True:
        byte, n = n & 0x7F, n >> 7
        if n:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def _multihash(digest):
    return b'\x12\x20' + digest


def _cid_v1_bytes(codec, digest):
    return _varint(1) + _varint(codec) + _multihash(digest)


def _base32(data):
    return 'b' + base64.b32encode(data).decode().rstrip('=').lower()


def _base58(data):
    n = int.from_bytes(data, 'big')
    out = ''
    while n:
        n, r = divmod(n, 58)
        out = B58[r] + out
    return '1' * (len(data) - len(data.lstrip(b'\0'))) + out


def _field(tag, payload):
    return bytes([tag]) + _varint(len(payload)) + payload


def _pb_node(data, links):
    """dag-pb: links (hash, empty name, tsize) before data, as unixfs.rs::encode_pb_node."""
    out = b''.join(_field(0x12, _field(0x0A, h) + b'\x12\x00' + b'\x18' + _varint(t)) for h, t in links)
    return out + _field(0x0A, data)


def _unixfs_leaf(chunk):
    return b'\x08\x02' + (_field(0x12, chunk) if chunk else b'') + b'\x18' + _varint(len(chunk))


def _unixfs_file(filesize, blocksizes):
    return b'\x08\x02\x18' + _varint(filesize) + b''.join(b'\x20' + _varint(b) for b in blocksizes)


@dataclasses.dataclass
class _Node:
    cid: bytes
    file_size: int
    tsize: int


class _Tree:
    """Balanced UnixFS reducer retaining fewer than MAX_CHILDREN nodes per level."""
    def __init__(self, cid_of):
        self.cid_of = cid_of
        self.levels = [[]]

    def add(self, node, level=0):
        if level == len(self.levels):
            self.levels.append([])
        self.levels[level].append(node)
        if len(self.levels[level]) == MAX_CHILDREN:
            self.promote(level)

    def promote(self, level):
        batch, self.levels[level] = self.levels[level], []
        size = sum(c.file_size for c in batch)
        block = _pb_node(_unixfs_file(size, [c.file_size for c in batch]),
                         [(c.cid, c.tsize) for c in batch])
        self.add(_Node(self.cid_of(hashlib.sha256(block).digest()), size,
                       len(block) + sum(c.tsize for c in batch)), level + 1)

    def finish(self, tail):
        tree = _Tree(self.cid_of)
        tree.levels = [list(level) for level in self.levels]
        tree.add(tail)
        while sum(map(len, tree.levels)) > 1:
            tree.promote(next(i for i, level in enumerate(tree.levels) if level))
        return next(level[0] for level in tree.levels if level)


class Hasher:
    """Every hash a stored key may be, over bytes fed in pieces: `hash_bytes_v1` (CIDv1, raw leaf
    up to one chunk, else a balanced dag-pb tree), `hash_bytes` (CIDv0 of the raw sha256),
    `hash_bytes_v0_unixfs` (CIDv0 as `ipfs add`) and the plain sha256 hex."""

    def __init__(self, unixfs_v0=False):
        self.sha = hashlib.sha256()
        self.pending = bytearray()
        self.v1_tree = _Tree(lambda digest: _cid_v1_bytes(0x70, digest))
        self.v0_tree = _Tree(_multihash)
        self.unixfs_v0 = unixfs_v0
        self.size = 0

    def _leaf(self, chunk):
        self.v1_tree.add(_Node(_cid_v1_bytes(0x55, hashlib.sha256(chunk).digest()), len(chunk), len(chunk)))
        if self.unixfs_v0:
            block = _pb_node(_unixfs_leaf(chunk), [])
            self.v0_tree.add(_Node(_multihash(hashlib.sha256(block).digest()), len(chunk), len(block)))

    def update(self, data):
        self.sha.update(data)
        self.size += len(data)
        view = memoryview(data)
        while view:
            if len(self.pending) == CHUNK_SIZE:
                self._leaf(bytes(self.pending))
                self.pending.clear()
            take = min(CHUNK_SIZE - len(self.pending), len(view))
            self.pending += view[:take]
            view = view[take:]

    def v1(self):
        if self.size <= CHUNK_SIZE:
            return _base32(_cid_v1_bytes(0x55, self.sha.digest()))
        tail = _Node(_cid_v1_bytes(0x55, hashlib.sha256(self.pending).digest()),
                     len(self.pending), len(self.pending))
        return _base32(self.v1_tree.finish(tail).cid)

    def v0(self):
        return _base58(_multihash(self.sha.digest()))

    def v0_unixfs(self):
        assert self.unixfs_v0
        # The last chunk is always pending: a leaf is cut only when more bytes follow it.
        block = _pb_node(_unixfs_leaf(bytes(self.pending)), [])
        tail = _Node(_multihash(hashlib.sha256(block).digest()), len(self.pending), len(block))
        return _base58(self.v0_tree.finish(tail).cid)

    def sha256_hex(self):
        return self.sha.hexdigest()

    def matches(self, key):
        """The CID this blob is stored under when its bytes hash to `key`, else None."""
        if SHA256_HEX.fullmatch(key):
            return self.v1() if self.sha256_hex() == key else None
        if key.startswith('Qm'):
            # Upstream stored both CIDv0 spellings (builder catalog.rs accepts both too).
            return key if key in (self.v0(), self.v0_unixfs() if self.unixfs_v0 else None) else None
        return key if self.v1() == key else None


def hash_bytes_v1(data):
    h = Hasher()
    h.update(data)
    return h.v1()


def hash_bytes_v0_unixfs(data):
    h = Hasher(unixfs_v0=True)
    h.update(data)
    return h.v0_unixfs()


# ── upstream storage (@dcl/catalyst-storage) ────────────────────────────────

class Missing(Exception):
    """Upstream does not serve the object; `str()` is the reason recorded in blob_missing."""


class SourceError(Exception):
    """An upstream failure that retrying one key does not fix (credentials, bucket, region)."""


class Denied(SourceError):
    """S3 refused one key before the probe showed the bucket readable."""


class ReadError(Exception):
    """Reading one object failed part-way (timeout, truncated stream); worth a retry."""


def shard(name):
    return hashlib.sha1(name.encode()).hexdigest()[:4]


class FolderSource:
    """A WCS `STORAGE_FOLDER` (or an `aws s3 sync` of its bucket): `<dir>/<shard>/<id>` by
    default, `<dir>/<id>` with `disablePrefixHash`; `<id>.gzip` is a gzip-only copy."""

    def __init__(self, root, *, exact_names=False):
        self.exact_names = exact_names
        self.root = Path(root)
        if not self.root.is_dir():
            raise FileNotFoundError(f'{root} is not a directory')

    def open(self, key):
        if self.exact_names:
            return self._open_exact(key)
        for path in (self.root / shard(key) / key, self.root / key):
            if path.is_file():
                return path.open('rb')
            if path.with_name(key + '.gzip').is_file():
                return gzip.open(path.with_name(key + '.gzip'), 'rb')
        raise Missing('absent upstream')

    def _open_exact(self, key):
        """Read a regular snapshot leaf without following links or blocking on a FIFO."""
        if not safe_filename(key):
            raise Missing('unsafe snapshot object name')
        try:
            directory = os.open(self.root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
            try:
                before = os.stat(key, dir_fd=directory, follow_symlinks=False)
                if not stat.S_ISREG(before.st_mode):
                    raise Missing('snapshot object is not a regular file')
                descriptor = os.open(key, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK,
                                     dir_fd=directory)
                try:
                    opened = os.fstat(descriptor)
                    current = os.stat(key, dir_fd=directory, follow_symlinks=False)
                    def identity(value):
                        return (value.st_dev, value.st_ino, value.st_mode, value.st_size,
                                value.st_mtime_ns, value.st_ctime_ns)
                    if (not stat.S_ISREG(opened.st_mode) or identity(before) != identity(opened)
                            or identity(opened) != identity(current)):
                        raise Missing('snapshot object changed while opening')
                    return os.fdopen(descriptor, 'rb')
                except BaseException:
                    os.close(descriptor)
                    raise
            finally:
                os.close(directory)
        except FileNotFoundError as error:
            raise Missing('absent upstream') from error
        except OSError as error:
            if error.errno in (errno.ELOOP, errno.ENOTDIR):
                raise Missing('snapshot path is a symlink or changed while opening') from error
            raise Missing('snapshot object cannot be safely opened') from error

    def describe(self):
        return str(self.root)


class S3Source:
    """A WCS `BUCKET`: the object key is the id. Needs boto3 (optional; credentials from the
    usual AWS environment)."""

    def __init__(self, bucket, prefix='', client=None):
        if client is None:
            try:
                import boto3  # noqa: PLC0415
            except ImportError as e:
                raise RuntimeError('reading S3 needs boto3 (pip install boto3), or `aws s3 sync` the bucket '
                                   'and pass --from-dir') from e
            client = boto3.client('s3')
        self.client = client
        self.bucket, self.prefix = bucket, prefix
        # Set once a probed key reads: a 403 then means the key is absent (no s3:ListBucket).
        self.readable = False

    def open(self, key):
        try:
            obj = self.client.get_object(Bucket=self.bucket, Key=self.prefix + key)
        except Exception as e:
            response = getattr(e, 'response', None)
            if not isinstance(response, dict):
                raise ReadError(f'{type(e).__name__}: {e}') from e  # connection or timeout
            code = str(response.get('Error', {}).get('Code', ''))
            status = response.get('ResponseMetadata', {}).get('HTTPStatusCode')
            if code in ('NoSuchKey', 'NotFound', '404') or status == 404:
                raise Missing('absent upstream') from e
            if code == 'RequestTimeTooSkewed':
                raise SourceError('S3 rejects requests signed with this machine\'s clock (RequestTimeTooSkewed);'
                                  ' sync the system clock and re-run') from e
            if code in S3_RETRYABLE or (isinstance(status, int) and status >= 500):
                raise ReadError(f'{code or status}: {e}') from e
            # Without s3:ListBucket a missing key answers 403 as well.
            if code in ('AccessDenied', 'Forbidden', '403') or status == 403:
                if not self.readable:
                    raise Denied(f's3://{self.bucket}/{self.prefix}{key}: access denied') from e
                raise Missing('access denied (absent upstream, or unreadable with these credentials)') from e
            raise SourceError(f's3://{self.bucket}/{self.prefix}{key}: {code or status}: {e}') from e
        body = obj['Body']
        if 'gzip' in (obj.get('ContentEncoding') or '').lower():
            return gzip.GzipFile(fileobj=body)
        return body

    def describe(self):
        return f's3://{self.bucket}/{self.prefix}'


# ── the node's store ────────────────────────────────────────────────────────

class Store:
    def __init__(self, storage_root):
        self.contents = Path(storage_root) / 'contents'

    def path(self, cid):
        return self.contents / shard(cid) / cid

    def has(self, cid, size=None):
        if not is_canonical_cid(cid):
            return False
        try:
            stat = self.path(cid).stat()
        except FileNotFoundError:
            return False
        return stat.st_size == size if size is not None else self.path(cid).is_file()

    def intact(self, cid):
        """The stored file's size when its bytes hash to `cid`, else None."""
        hasher = Hasher(unixfs_v0=cid.startswith('Qm'))
        try:
            with self.path(cid).open('rb') as fh:
                while chunk := fh.read(READ_SIZE):
                    hasher.update(chunk)
        except (FileNotFoundError, IsADirectoryError):
            return None
        return hasher.size if hasher.matches(cid) == cid else None

    def staging(self, key):
        """A new staging file (path, open binary file), never an existing file or a symlink."""
        directory = self.contents / shard(key)
        directory.mkdir(parents=True, exist_ok=True)
        path = directory / f'{key}.{os.getpid()}.{next(_seq)}.tmp'
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o644)
        return path, os.fdopen(fd, 'wb')

    def commit(self, staged, cid, replace=False):
        final = self.path(cid)
        if final.exists() and not replace:
            staged.unlink()
            return
        final.parent.mkdir(parents=True, exist_ok=True)
        os.replace(staged, final)
        fd = os.open(final.parent, os.O_RDONLY)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)


@dataclasses.dataclass
class Outcome:
    key: str
    status: str  # copied | present | missing | corrupt | invalid | failed
    cid: str | None = None
    size: int = 0
    reason: str | None = None


REASONS = {'corrupt': 'bytes do not hash to the key', 'invalid': 'not a content id'}


def _read_into(source, store, key):
    """One attempt: streams `key` into a staging file; returns (staged path, hasher)."""
    try:
        stream = source.open(key)
    except (Missing, SourceError, ReadError):
        raise
    except Exception as e:
        raise ReadError(f'{type(e).__name__}: {e}') from e
    staged, out = store.staging(hashlib.sha256(key.encode()).hexdigest())
    hasher = Hasher(unixfs_v0=key.startswith('Qm'))
    try:
        with contextlib.closing(stream), out:
            while True:
                try:
                    chunk = stream.read(READ_SIZE)
                except Exception as e:
                    raise ReadError(f'{type(e).__name__}: {e}') from e
                if not chunk:
                    break
                hasher.update(chunk)
                out.write(chunk)
            out.flush()
            os.fsync(out.fileno())
        return staged, hasher
    except BaseException:
        staged.unlink(missing_ok=True)
        raise


def copy_one(source, store, key, *, filename=False):
    """Copies upstream object `key` (a CID, or a sha256 hex thumbnail) and verifies its bytes."""
    if not (safe_filename(key) if filename else (is_canonical_cid(key) or SHA256_HEX.fullmatch(key))):
        return Outcome(key, 'invalid', reason=REASONS['invalid'])
    replace = False
    if not filename and is_canonical_cid(key) and store.has(key):
        size = store.intact(key)
        if size is not None:
            return Outcome(key, 'present', key, size)
        replace = True  # a truncated or foreign file under this name
    for attempt in range(READ_ATTEMPTS):
        try:
            staged, hasher = _read_into(source, store, key)
            break
        except Missing as e:
            return Outcome(key, 'missing', reason=str(e))
        except ReadError as e:
            if attempt + 1 == READ_ATTEMPTS:
                return Outcome(key, 'failed', reason=f'read failed {READ_ATTEMPTS} times: {e}')
            time.sleep(RETRY_DELAY * 2 ** attempt)
    try:
        cid = hasher.v1() if filename else hasher.matches(key)
        if cid is None:
            staged.unlink()
            return Outcome(key, 'corrupt', reason=REASONS['corrupt'])
        # SHA-256 thumbnail keys resolve to a CID only after hashing. Check
        # that destination too, so a retry repairs an existing corrupt file.
        if cid != key and store.has(cid):
            replace = store.intact(cid) is None
        store.commit(staged, cid, replace=replace)
        return Outcome(key, 'copied', cid, hasher.size)
    except BaseException:
        staged.unlink(missing_ok=True)
        raise


@dataclasses.dataclass
class Probe:
    readable: int
    unreadable: dict  # key -> reason


def _spread(keys, n):
    keys = [k for k in keys if is_canonical_cid(k) or SHA256_HEX.fullmatch(k)]
    return keys if len(keys) <= n else [keys[i * len(keys) // n] for i in range(n)]


def probe(source, keys, n=PROBE_KEYS, reference=(), n_reference=PROBE_REFERENCE_KEYS):
    """Opens up to `n` keys spread over `keys`, and up to `n_reference` over `reference` (keys an
    earlier run read): a wrong folder or credentials make every one unreadable. Marks an S3 source
    readable when one opens."""
    sample = _spread(keys, n)
    sample += [k for k in _spread(reference, n_reference) if k not in sample]
    result = Probe(0, {})
    for key in sample:
        try:
            stream = source.open(key)
        except (Missing, Denied, ReadError) as e:
            result.unreadable[key] = str(e)
            continue
        stream.close()
        result.readable += 1
    if result.readable:
        source.readable = True
    return result


def copy_all(source, store, keys, workers, window=None, copy=copy_one):
    """`copy` (`copy_one`) over `keys` in order, with at most `window` keys submitted at a time."""
    window = window or workers * 4
    keys = iter(keys)
    pool = ThreadPoolExecutor(max_workers=workers)
    try:
        pending = collections.deque(pool.submit(copy, source, store, k) for k in itertools.islice(keys, window))
        while pending:
            outcome = pending.popleft().result()
            for k in itertools.islice(keys, 1):
                pending.append(pool.submit(copy, source, store, k))
            yield outcome
    finally:
        pool.shutdown(wait=True, cancel_futures=True)


# ── badge art (upstream CDN_HOST → BADGES_ASSETS_DIR) ──────────────────────
#
# Badge art is addressed by path, not by hash: the catalog's `assets` documents hold
# `<badge>[/<tier slot>]/{2d,3d}/<texture>.png`, relative to upstream's CDN_HOST, and the node
# serves `BADGES_ASSETS_DIR/<same path>`. Each file is written through a staging file and a rename.

# Upstream's production CDN_HOST, the node's default asset base.
BADGE_ART_BASE = 'https://assets-cdn.decentraland.org/'
ART_PATH = re.compile(r'[a-z0-9_-]+(/[a-z0-9_-]+)*/(2d|3d)/[a-z0-9_-]+\.png')
PNG_MAGIC = b'\x89PNG\r\n\x1a\n'
HTTP_TIMEOUT = 30


def art_path(value, base=BADGE_ART_BASE):
    """The badge texture path an `assets` value names: a relative path, or a URL under `base`; else
    None (an empty slot, or art hosted elsewhere, such as the node's embedded seed art)."""
    if not isinstance(value, str):
        return None
    path = value[len(base):] if value.startswith(base) else value
    return path if ART_PATH.fullmatch(path) else None


class HttpArtSource:
    """Upstream's CDN: `GET <base><path>`."""

    def __init__(self, base=BADGE_ART_BASE, opener=urllib.request.urlopen):
        self.base = base if base.endswith('/') else base + '/'
        self.opener = opener

    def open(self, path):
        try:
            return self.opener(urllib.request.Request(self.base + path, headers={'User-Agent': 'dcl-migration'}),
                               timeout=HTTP_TIMEOUT)
        except urllib.error.HTTPError as e:
            e.close()
            if e.code in (403, 404, 410):
                raise Missing(f'HTTP {e.code} upstream') from e
            if e.code == 429 or e.code >= 500:
                raise ReadError(f'HTTP {e.code}') from e
            raise SourceError(f'{self.base}{path}: HTTP {e.code}') from e
        except (urllib.error.URLError, TimeoutError, ConnectionError) as e:
            raise ReadError(f'{type(e).__name__}: {e}') from e

    def describe(self):
        return self.base


class FolderArtSource:
    """A local copy of the CDN bucket's badge art, laid out by path."""

    def __init__(self, root):
        self.root = Path(root)
        if not self.root.is_dir():
            raise FileNotFoundError(f'{root} is not a directory')

    def open(self, path):
        try:
            return (self.root / path).open('rb')
        except (FileNotFoundError, IsADirectoryError, NotADirectoryError) as e:
            raise Missing('absent upstream') from e

    def describe(self):
        return str(self.root)


class ArtDir:
    """The node's `BADGES_ASSETS_DIR`."""

    def __init__(self, root, refresh=False):
        self.root = Path(root)
        self.refresh = refresh

    def path(self, path):
        return self.root / path

    def has(self, path):
        try:
            return self.path(path).stat().st_size > 0
        except (FileNotFoundError, NotADirectoryError):
            return False


def copy_art(source, art_dir, path):
    """Copies one badge texture into `art_dir` unless it is already there (or `art_dir.refresh`)."""
    if not ART_PATH.fullmatch(path):
        return Outcome(path, 'invalid', reason='not a badge texture path')
    final = art_dir.path(path)
    if art_dir.has(path) and not art_dir.refresh:
        return Outcome(path, 'present', path, final.stat().st_size)
    for attempt in range(READ_ATTEMPTS):
        try:
            stream = source.open(path)
            with contextlib.closing(stream):
                data = stream.read()
            break
        except Missing as e:
            return Outcome(path, 'missing', reason=str(e))
        except (ReadError, OSError) as e:
            if attempt + 1 == READ_ATTEMPTS:
                return Outcome(path, 'failed', reason=f'read failed {READ_ATTEMPTS} times: {e}')
            time.sleep(RETRY_DELAY * 2 ** attempt)
    if not data.startswith(PNG_MAGIC):
        return Outcome(path, 'corrupt', reason='not a PNG')
    final.parent.mkdir(parents=True, exist_ok=True)
    staged = final.parent / f'.{final.name}.{os.getpid()}.{next(_seq)}.tmp'
    fd = os.open(staged, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o644)
    try:
        with os.fdopen(fd, 'wb') as out:
            out.write(data)
            out.flush()
            os.fsync(out.fileno())
        os.replace(staged, final)
    except BaseException:
        staged.unlink(missing_ok=True)
        raise
    return Outcome(path, 'copied', path, len(data))


# Camera object keys are immutable UUID filenames, not content hashes. Only
# explicit filename mode may compute a new CID without matching the source key.
def safe_filename(key):
    return bool(key and len(key.encode()) <= 255 and key not in ('.', '..')
                and not any(c in key for c in '/\\')
                and all(ord(c) >= 32 and ord(c) != 127 for c in key))


def copy_filename(source, store, key):
    return copy_one(source, store, key, filename=True)


def verify_publication(inventory, storage_root):
    """Client-side psql backslash-copy helper. The parent psql holds the GC shared lock.

    Fail before emitting any verification receipt if a staged CID vanished or
    changed. No source access, image transformation, SQL, or network I/O.
    """
    store = Store(storage_root)
    checked = []
    with open(inventory, newline='') as stream:
        for cid, size in csv.reader(stream):
            if not is_canonical_cid(cid) or store.intact(cid) != int(size):
                raise RuntimeError('staged blob missing or corrupt; recopy blobs before resuming')
            checked.append((cid, size))
    csv.writer(sys.stdout).writerows(checked)


if __name__ == '__main__':
    if len(sys.argv) != 4 or sys.argv[1] != 'verify-publication':
        raise SystemExit('usage: blobs.py verify-publication INVENTORY STORAGE_ROOT')
    verify_publication(sys.argv[2], sys.argv[3])
