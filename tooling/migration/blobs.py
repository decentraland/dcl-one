"""Copies the blobs a source's imported rows name into the node's blob store (`migrate.py blobs`).

The store layout is `services/catalyst/storage`'s: `<STORAGE_ROOT_FOLDER>/contents/<shard>/<cid>`,
shard = the first four hex digits of sha1(cid), written through a `<name>.<pid>.<seq>.tmp` staging
file and a rename (the node's startup sweep removes a staging file a crash leaves behind). Every
object's bytes are re-hashed before they are stored; the CID functions reproduce
`library/dcl-auth/src/hash.rs`.
"""
import base64
import contextlib
import dataclasses
import gzip
import hashlib
import itertools
import os
import re
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

CHUNK_SIZE = 262_144
MAX_CHILDREN = 174
READ_SIZE = 1 << 20
CIDV0 = re.compile(r'^Qm[1-9A-HJ-NP-Za-km-z]{44}$')
CIDV1 = re.compile(r'^ba[a-z2-7]{56,98}$')
SHA256_HEX = re.compile(r'^[0-9a-f]{64}$')
B58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
_seq = itertools.count()


# ── CIDs (library/dcl-auth/src/{hash,cid,unixfs}.rs) ────────────────────────

def is_canonical_cid(value):
    return bool(CIDV0.match(value) or CIDV1.match(value))


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


def _reduce(nodes, cid_of):
    while len(nodes) > 1:
        parents = []
        for i in range(0, len(nodes), MAX_CHILDREN):
            batch = nodes[i:i + MAX_CHILDREN]
            size = sum(c.file_size for c in batch)
            block = _pb_node(_unixfs_file(size, [c.file_size for c in batch]), [(c.cid, c.tsize) for c in batch])
            parents.append(_Node(cid_of(hashlib.sha256(block).digest()), size,
                                 len(block) + sum(c.tsize for c in batch)))
        nodes = parents
    return nodes[0]


class Hasher:
    """Every hash a stored key may be, over bytes fed in pieces: `hash_bytes_v1` (CIDv1, raw leaf
    up to one chunk, else a balanced dag-pb tree), `hash_bytes` (CIDv0 of the raw sha256),
    `hash_bytes_v0_unixfs` (CIDv0 as `ipfs add`) and the plain sha256 hex."""

    def __init__(self, unixfs_v0=False):
        self.sha = hashlib.sha256()
        self.pending = bytearray()
        self.v1_leaves, self.v0_leaves = [], []
        self.unixfs_v0 = unixfs_v0
        self.size = 0

    def _leaf(self, chunk):
        self.v1_leaves.append(_Node(_cid_v1_bytes(0x55, hashlib.sha256(chunk).digest()), len(chunk), len(chunk)))
        if self.unixfs_v0:
            block = _pb_node(_unixfs_leaf(chunk), [])
            self.v0_leaves.append(_Node(_multihash(hashlib.sha256(block).digest()), len(chunk), len(block)))

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
        leaves = list(self.v1_leaves)
        if self.pending:
            leaves.append(_Node(_cid_v1_bytes(0x55, hashlib.sha256(self.pending).digest()),
                                len(self.pending), len(self.pending)))
        return _base32(_reduce(leaves, lambda d: _cid_v1_bytes(0x70, d)).cid)

    def v0(self):
        return _base58(_multihash(self.sha.digest()))

    def v0_unixfs(self):
        assert self.unixfs_v0
        # The last chunk is always pending: a leaf is cut only when more bytes follow it.
        block = _pb_node(_unixfs_leaf(bytes(self.pending)), [])
        tail = _Node(_multihash(hashlib.sha256(block).digest()), len(self.pending), len(block))
        return _base58(_reduce(self.v0_leaves + [tail], _multihash).cid)

    def sha256_hex(self):
        return self.sha.hexdigest()

    def matches(self, key):
        """The CID this blob is stored under when its bytes hash to `key`, else None."""
        if SHA256_HEX.match(key):
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

def shard(name):
    return hashlib.sha1(name.encode()).hexdigest()[:4]


class FolderSource:
    """A WCS `STORAGE_FOLDER` (or an `aws s3 sync` of its bucket): `<dir>/<shard>/<id>` by
    default, `<dir>/<id>` with `disablePrefixHash`; `<id>.gzip` is a gzip-only copy."""

    def __init__(self, root):
        self.root = Path(root)
        if not self.root.is_dir():
            raise FileNotFoundError(f'{root} is not a directory')

    def open(self, key):
        for path in (self.root / shard(key) / key, self.root / key):
            if path.is_file():
                return path.open('rb')
            if path.with_name(key + '.gzip').is_file():
                return gzip.open(path.with_name(key + '.gzip'), 'rb')
        return None

    def describe(self):
        return str(self.root)


class S3Source:
    """A WCS `BUCKET`: the object key is the id. Needs boto3 (optional; credentials from the
    usual AWS environment)."""

    def __init__(self, bucket, prefix=''):
        try:
            import boto3  # noqa: PLC0415
        except ImportError as e:
            raise RuntimeError('reading S3 needs boto3 (pip install boto3), or `aws s3 sync` the bucket '
                               'and pass --from-dir') from e
        self.client = boto3.client('s3')
        self.bucket, self.prefix = bucket, prefix

    def open(self, key):
        try:
            obj = self.client.get_object(Bucket=self.bucket, Key=self.prefix + key)
        except self.client.exceptions.NoSuchKey:
            return None
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

    def has(self, cid):
        return is_canonical_cid(cid) and self.path(cid).is_file()

    def staging(self, key):
        directory = self.contents / shard(key)
        directory.mkdir(parents=True, exist_ok=True)
        return directory / f'{key}.{os.getpid()}.{next(_seq)}.tmp'

    def commit(self, staged, cid):
        final = self.path(cid)
        if final.exists():
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
    status: str  # copied | present | missing | corrupt | invalid
    cid: str | None = None
    size: int = 0


def copy_one(source, store, key):
    """Copies upstream object `key` (a CID, or a sha256 hex thumbnail) and verifies its bytes."""
    if not (is_canonical_cid(key) or SHA256_HEX.match(key)):
        return Outcome(key, 'invalid')
    if is_canonical_cid(key) and store.has(key):
        return Outcome(key, 'present', key, store.path(key).stat().st_size)
    stream = source.open(key)
    if stream is None:
        return Outcome(key, 'missing')
    staged = store.staging(key)
    hasher = Hasher(unixfs_v0=key.startswith('Qm'))
    try:
        with contextlib.closing(stream), staged.open('wb') as out:
            while chunk := stream.read(READ_SIZE):
                hasher.update(chunk)
                out.write(chunk)
            out.flush()
            os.fsync(out.fileno())
        cid = hasher.matches(key)
        if cid is None:
            staged.unlink()
            return Outcome(key, 'corrupt')
        store.commit(staged, cid)
        return Outcome(key, 'copied', cid, hasher.size)
    except BaseException:
        staged.unlink(missing_ok=True)
        raise


def copy_all(source, store, keys, workers):
    with ThreadPoolExecutor(max_workers=workers) as pool:
        yield from pool.map(lambda k: copy_one(source, store, k), keys)
