#!/usr/bin/env python3
"""Rebuild the patched third-party Rust sources from an upstream reference and a patch series.

Tracked, per source:  rendering/bevy-explorer/patches/<name>/source.toml
                      rendering/bevy-explorer/patches/<name>/series/NNNN-<slug>.patch
Written here, ignored by git: the patched tree, called the dest, at rendering/bevy-explorer/bevy for
`bevy` and rendering/bevy-explorer/forks/<name> for every other source, and <dest>/.patched-stamp.json.

  materialise [NAME...] [--offline] [--base-dir DIR] [--dest-root DIR] [--force]
  verify      [NAME...]                    base_tree, tree and rev of source.toml reproduce; exit 1 if not
  status      [NAME...]                    missing | current | stale | edited, one line per source
  export      NAME -m "subject" [--binary] the edits in the dest become the next patch; tree and rev are updated
  divergence  [NAME...]                    files and lines changed against the base, by crate
  repin       NAME...                      base_tree, tree and rev are written again, and the rev in the two Cargo files
  pins                                     the Explorer names the rev of every source; exit 1 if not
  publish     [NAME...] [--remote URL] [--check [--fetchable]]  push the commit of each source that the remote lacks
  edit        NAME [--stop]                the Explorer builds one source from its dest; --stop ends that

`--root DIR` before the verb works on the checkout DIR instead of the one that holds this tool, so one
copy of the tool serves each checkout.

source.toml: name, upstream and commit (the base), ref, origin and origin_rev (where the series came
from), license (an SPDX expression), paths and exclude (the part of the base that is kept), binary
(the patch files that may hold a binary hunk), base_tree, tree and rev (what must reproduce), nix_hash,
and one [[submodule]] with path, url, commit and nix_hash for each gitlink that is filled.

Trees are built in a scratch git index (read-tree, apply --cached, write-tree) of a bare repository,
so bytes are exact on every platform: no line-ending conversion, no attributes. Bases are fetched
once, by commit, into $DCL_PATCHED_SOURCES_CACHE (default ~/.cache/dcl-one/patched-sources), one bare repository
per source. `--base-dir DIR` reads an exported base from DIR/<name>/ and needs no network;
`--dest-root DIR` writes below DIR instead of the repository root.

A dest holds plain files only. A link in the tree is written as a copy of the file it names, or as
its target text when it names no file of the tree. Each `[[submodule]]` path is filled from its url
and commit, and the tree must hold a gitlink to that commit at the path. `.git` and `target` at the
top of a dest are never read, written or removed.

materialise writes a dest only when base_tree and tree of source.toml reproduce and, when it has a
rev, the files to write make the commit with that id.

A tree that holds a file under .github/workflows/ fails every command that builds it: GitHub refuses
the push of such a commit from a workflow's token. A source excludes `.github`.

materialise keeps an edited dest. While the series is the one the dest was written from, it says
`edited, kept` and writes nothing. When the series changed, it rebuilds the new tree and merges the
edits into it, file by file against the tree the dest was written from, which the cache keeps under
refs/patched-sources. A conflict changes nothing in the dest. The edits are saved as a patch below
the cache in both cases. `--force` drops the edits.

The Explorer takes each source as a Cargo git dependency on a commit of MIRROR, which Cargo.toml and
Cargo.lock of rendering/bevy-explorer/source name by its id, the rev of source.toml. That commit has
no parent, the tree of the dest as materialise writes it, the author and committer
`patched-sources <patched-sources@invalid>` at time 0 and the message `patched source <name>`: the rev
follows from the base and the series alone. publish lists the refs of the remote, skips each source
whose ref is there with no rebuild, builds the commit of each other source in the cache repository and
pushes it to refs/patched-sources/<name>/<rev>. It never deletes and never forces. It talks to the
remote with the git configuration of the caller, so the caller's credentials apply. `--check` pushes
nothing and lists what the remote lacks. `--fetchable` with `--check` also passes a source whose ref
the remote lacks when the remote serves its rev by id: GitHub serves through a repository the commits
of each of its forks, so a contributor's `publish --remote <fork>` lets Cargo fetch the rev from MIRROR
before the merge. `--remote URL` names another repository than MIRROR: a fork, or a trial push.

pins reads the two Cargo files and no repository: every entry and package that names MIRROR names the
rev of a source, every rev is in both files, and no entry reads a dest by path. Every other git URL
of the two files is, letter for letter, one of OTHER_GIT, and has none of those crates. A package of
the lock file from such a URL is at the commit that OTHER_GIT gives. MIRROR in another spelling (other
case, `.git` or `/` at the end) fails.

edit NAME materialises the dest and makes the Explorer build it: the entries of Cargo.toml that name
the rev become path entries, and in Cargo.lock the packages of the rev lose their source, so
`cargo --locked` accepts both. `edit --stop NAME` puts the rev back in both files. export does the
same with the new rev; outside an edit it replaces the old rev by the new one.

export writes the patch, then source.toml, then Cargo.toml, then Cargo.lock. A run that stops after
source.toml leaves there a rev that a Cargo file does not name, and says so. `repin NAME` writes that
rev in both files, and so does `edit --stop NAME`.

repin NAME... rebuilds each source from its upstream commit and its series, and writes what follows
from them: base_tree, tree and rev in source.toml, and the rev in Cargo.toml and Cargo.lock. It is
the step after a change by hand to the series, to `commit` or to the part of the base that is kept,
and after a run that stopped between its writes. The Cargo files may name for the source its dest (an
edit, which repin ends), the rev that source.toml has, or a rev that no source.toml has and that only
crates of the source name. repin writes no entry and no package that the files lack: it fails and
prints the text to write. It reads no dest and writes none, and it does not write nix_hash.

One run at a time works on a dest, on a cache repository and on the two Cargo files: each has a lock
file beside it, held while the run works and removed after.

The stamp is one JSON line:
  {"crc32": ..., "executable": [...], "files": ..., "local": [...], "tree": ..., "written": ...}
crc32 and files cover the same byte stream: for each file in path order, the path, a zero byte, the
length as 8 little-endian bytes, the bytes. crc32 = that stream over the source's source.toml and
patches, files = sha256 of it over the dest as written, executable = the files written with the
executable bit, local = names in the dest that are not its content, tree = the git tree of the
series, written = the git tree of the files as written.
"""
import sys

if sys.version_info < (3, 11):
    sys.exit("patched-sources: Python 3.11 or newer is needed, and this is Python %d.%d" % sys.version_info[:2])

import argparse
import collections
import contextlib
import datetime
import fnmatch
import hashlib
import itertools
import json
import os
from pathlib import Path
import posixpath
import re
import subprocess
import tempfile
import time
import tomllib
import zlib

ROOT = Path(__file__).resolve().parents[1]
EXPLORER = "rendering/bevy-explorer"
PATCHES = EXPLORER + "/patches"
WORKSPACE = EXPLORER + "/source"
MIRROR = "https://github.com/decentraland/dcl-one"
# The other git repositories that the Explorer takes a crate from, each with the commit of the lock file.
OTHER_GIT = {"https://github.com/gltf-rs/gltf": "12fc1b7ce28c7b4c6d1b881a30df24060dccfbda"}
IDENT = "patched-sources <patched-sources@invalid> 0 +0000"
STAMP = ".patched-stamp.json"
LOCAL = frozenset({".git", "target", STAMP})
JUNK = ("*.orig", "*.rej", "*~", "*.swp", ".DS_Store")
KEYS = frozenset({"name", "upstream", "commit", "ref", "origin", "origin_rev", "license", "paths", "exclude",
                  "binary", "base_tree", "tree", "rev", "nix_hash", "submodule"})
SUBMODULE_KEYS = frozenset({"path", "url", "commit", "nix_hash"})
ZERO = "0" * 40
OID = re.compile(r"[0-9a-f]{40}")
REF = re.compile(r"[A-Za-z0-9][A-Za-z0-9._+-]*(/[A-Za-z0-9][A-Za-z0-9._+-]*)*")
SRI = re.compile(r"sha256-[A-Za-z0-9+/]{43}=")
LICENSE_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9.-]*\+?")
BINARY_HUNK = re.compile(rb"(?m)^GIT binary patch$")
LOCK_FIELD = re.compile(r'(name|version|source) = "([^"]*)"')
LOCK_DEPENDENCY = re.compile(r' "([^" ]+)(?: ([^" ]+)(?: \(([^"]+)\))?)?",')
URL_SCHEMES = ("https://",)
FETCH_SECONDS = 300
FETCH_ATTEMPTS = 3
FETCH_PAUSE = 5
# The fetch of one commit without its tree that `publish --check --fetchable` makes for each source without a ref.
PROBE_SECONDS = 60
PUSH_SECONDS = 1200
LOCK_SECONDS = 1200


class Failure(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise Failure(message)


def note(message):
    print(f"patched-sources: {message}", file=sys.stderr)


def chunks(files):
    for name, path in sorted(files):
        data = content(path)
        yield name.encode("utf-8") + b"\0" + len(data).to_bytes(8, "little")
        yield data


def sha256(files):
    digest = hashlib.sha256()
    for chunk in chunks(files):
        digest.update(chunk)
    return digest.hexdigest()


def crc32(files):
    value = 0
    for chunk in chunks(files):
        value = zlib.crc32(chunk, value)
    return f"{value:08x}"


def blob_id(data):
    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()


def content(path):
    return os.fsencode(os.readlink(path)) if path.is_symlink() else path.read_bytes()


def walk(directory, skip=()):
    """(relative POSIX path, Path) of every file below a directory; links are entries, never followed."""
    found, pending = [], [(Path(directory), "")]
    while pending:
        current, prefix = pending.pop()
        if not current.is_dir():
            continue
        with os.scandir(current) as entries:
            for entry in entries:
                relative = prefix + entry.name
                if relative in skip:
                    continue
                if entry.is_dir(follow_symlinks=False):
                    pending.append((Path(entry.path), relative + "/"))
                else:
                    found.append((relative, Path(entry.path)))
    return sorted(found)


def file_mode(path, known="100644"):
    if path.is_symlink():
        return "120000"
    if os.name == "nt":
        return known if known in ("100644", "100755") else "100644"
    return "100755" if path.stat().st_mode & 0o100 else "100644"


def scan(dest, skip, known=None):
    """{path: (mode, blob id)} of the files in a dest. Windows keeps the mode a file is known by."""
    known = known or {}
    return {name: (file_mode(path, known.get(name, ("",))[0]), blob_id(content(path))) for name, path in walk(dest, skip)}


def compare(rows, disk):
    """(changed, missing, added) paths of a dest against the rows it was written from."""
    return (sorted(path for path in rows if path in disk and disk[path] != rows[path]),
            sorted(path for path in rows if path not in disk),
            sorted(path for path in disk if path not in rows))


def counted(changed, missing, added):
    return f"{len(changed)} changed, {len(missing)} missing, {len(added)} added"


def replace(temporary, path):
    # On Windows the rename fails while a reader holds the file open.
    for attempt in range(5):
        try:
            os.replace(temporary, path)
            return
        except PermissionError:
            if attempt == 4:
                raise
            time.sleep(0.1 * (attempt + 1))


def write_atomic(path, data, temporary=None, sync=False):
    """Readers see the old file or the new one, never a part of it."""
    temporary = temporary or path.with_name(path.name + ".tmp")
    try:
        with open(temporary, "wb") as handle:
            handle.write(data)
            if sync:
                handle.flush()
                os.fsync(handle.fileno())
        replace(temporary, path)
    except BaseException:
        with contextlib.suppress(OSError):
            temporary.unlink()
        raise


def try_lock(handle):
    try:
        if os.name == "nt":
            import msvcrt
            handle.seek(0)
            msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
        else:
            import fcntl
            fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        return False
    return True


@contextlib.contextmanager
def locked(path, label):
    """Hold the lock file at a path. The system drops the lock of a run that died, so none goes stale."""
    path.parent.mkdir(parents=True, exist_ok=True)
    deadline, pause = time.monotonic() + LOCK_SECONDS, 0.05
    while True:
        handle = open(path, "a+b")
        if try_lock(handle):
            # The run before removed the file it held: the lock counts only on the file the path names now.
            try:
                if os.path.samestat(os.fstat(handle.fileno()), os.stat(path)):
                    break
            except OSError:
                pass
        handle.close()
        require(time.monotonic() < deadline,
                f"{label}: another run holds the lock {path}; gave up after {LOCK_SECONDS} seconds")
        time.sleep(pause)
        pause = min(pause * 2, 1.0)
    try:
        yield
    finally:
        # Windows removes no open file; everywhere else the file goes while the lock still holds.
        if os.name != "nt":
            with contextlib.suppress(OSError):
                path.unlink()
        handle.close()
        if os.name == "nt":
            with contextlib.suppress(OSError):
                path.unlink()


def git_environment(index=None):
    # Inherited repository, index and config redirects must not change what is built.
    environment = {key: value for key, value in os.environ.items()
                   if not key.startswith("GIT_") or key.startswith(("GIT_SSL_", "GIT_HTTP_"))}
    # In another locale git translates its messages, and with them the prefix that marks a hint.
    environment.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
                       GIT_TERMINAL_PROMPT="0", GCM_INTERACTIVE="never", LC_ALL="C")
    if index is not None:
        environment["GIT_INDEX_FILE"] = str(index)
    return environment


def remote_environment():
    # A push needs the credentials of the caller, so its configuration is read. The id fixes what is pushed.
    environment = git_environment()
    del environment["GIT_CONFIG_NOSYSTEM"], environment["GIT_CONFIG_GLOBAL"]
    environment.update({key: value for key, value in os.environ.items()
                        if key.startswith(("GIT_CONFIG_", "GIT_ASKPASS", "GIT_SSH"))})
    return environment


class Repo:
    """A bare repository: the object store of one source. Every tree lives in a scratch index."""
    OPTIONS = ("-c", "protocol.ext.allow=never", "-c", "core.autocrlf=false", "-c", "core.safecrlf=false",
               "-c", "core.longpaths=true", "-c", "gc.auto=0", "-c", "maintenance.auto=false")

    def __init__(self, path):
        self.path = Path(path)
        if not (self.path / "HEAD").is_file():
            self.path.mkdir(parents=True, exist_ok=True)
            # The ids of source.toml are SHA-1 ids, whatever format this git makes a repository in when none is named.
            self.run(["git", "init", "-q", "--bare", "--template=", "--object-format=sha1", str(self.path)])

    def command(self, arguments):
        return ["git", *self.OPTIONS, "--git-dir", str(self.path), *arguments]

    @staticmethod
    def run(command, index=None, stdin=None, cwd=None, timeout=None, ok=(0,), environment=None):
        """Run git; `ok` lists the exit codes that are no failure, None takes every code."""
        try:
            result = subprocess.run(command, input=stdin, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                    env=environment or git_environment(index), cwd=cwd, timeout=timeout)
        except FileNotFoundError as error:
            raise Failure("the git executable is not on PATH") from error
        except subprocess.TimeoutExpired as error:
            raise Failure(f"git exceeded {timeout} seconds") from error
        if ok is not None and result.returncode not in ok:
            # The hints of git say what to do next in a checkout, never what failed.
            lines = [line for line in result.stderr.decode("utf-8", "replace").strip().splitlines()
                     if not line.startswith("hint:")]
            raise Failure("; ".join(lines[-3:]) or f"git exit {result.returncode}")
        return result

    def git(self, *arguments, **options):
        return self.run(self.command(arguments), **options).stdout

    def text(self, *arguments, **options):
        return self.git(*arguments, **options).decode("ascii").strip()

    def has(self, oid, kind):
        return self.run(self.command(["cat-file", "-e", f"{oid}^{{{kind}}}"]), ok=(0, 1, 128)).returncode == 0

    def fetch(self, url, commit, offline, label):
        if self.has(commit, "commit"):
            return
        require(not offline, f"{label}: commit {commit} is not in {self.path} and --offline forbids a fetch")
        for attempt in range(1, FETCH_ATTEMPTS + 1):
            try:
                # Nothing after --end-of-options is read as an option, whatever source.toml holds.
                self.git("fetch", "-q", "--no-tags", "--depth", "1", "--end-of-options", url, commit, timeout=FETCH_SECONDS)
                break
            except Failure as error:
                if attempt == FETCH_ATTEMPTS:
                    raise Failure(f"{label}: cannot fetch {commit} from {url} in {attempt} attempts: {error}") from error
                pause = FETCH_PAUSE * 3 ** (attempt - 1)
                note(f"{label}: fetch attempt {attempt} of {FETCH_ATTEMPTS} failed ({error}); "
                     f"the next one starts in {pause} seconds")
                time.sleep(pause)
        require(self.has(commit, "commit"), f"{label}: {url} did not deliver commit {commit}")

    @staticmethod
    def records(listing, oid_field):
        result = []
        for record in filter(None, listing.split(b"\0")):
            meta, path = record.split(b"\t", 1)
            fields = meta.decode("ascii").split()
            result.append((fields[0], fields[oid_field], path.decode("utf-8")))
        return result

    def rows(self, index, *pathspec):
        """(mode, oid, path) of the index entries, all or those a pathspec selects."""
        return self.records(self.git("ls-files", "-s", "-z", "--", *pathspec, index=index), 1)

    def tree_rows(self, tree):
        return self.records(self.git("ls-tree", "-r", "-z", tree), 2)

    def update(self, index, rows):
        """Add, replace or (oid of zeros) remove index entries."""
        lines = b"".join(f"{'0' if oid == ZERO else mode} {oid} 0\t".encode("ascii") + path.encode("utf-8") + b"\0"
                         for mode, oid, path in rows)
        self.git("update-index", "-z", "--index-info", index=index, stdin=lines)

    def load(self, index, rows):
        self.git("read-tree", "--empty", index=index)
        self.update(index, rows)

    def store(self, files, known=None):
        """Write files as blobs, bytes as they are; (mode, oid, path) rows."""
        known = known or {}
        plain = [(name, path) for name, path in files if not path.is_symlink()]
        for _name, path in plain:
            require("\n" not in str(path) and not str(path).startswith('"'), f"unsupported file name: {path}")
        listing = "".join(f"{path}\n" for _name, path in plain).encode("utf-8")
        oids = self.git("hash-object", "-w", "--no-filters", "--stdin-paths", stdin=listing).decode("ascii").split()
        require(len(oids) == len(plain), "git hash-object returned an incomplete list")
        rows = [(file_mode(path, known.get(name)), oid, name) for (name, path), oid in zip(plain, oids)]
        for name, path in files:
            if path.is_symlink():
                rows.append(("120000", self.text("hash-object", "-w", "--stdin", stdin=content(path)), name))
        return sorted(rows, key=lambda row: row[2])

    def write_tree(self, index):
        return self.text("write-tree", index=index)

    def tree_of(self, index, files):
        """The tree of {path: (mode, oid)}, written through an index that is overwritten."""
        self.load(index, [(mode, oid, path) for path, (mode, oid) in files.items()])
        return self.write_tree(index)

    def keep(self, tree):
        """Name a tree, so that no collection of the cache drops what an edited dest is compared with."""
        self.git("update-ref", f"refs/patched-sources/{tree}", tree)

    def diff(self, old, new):
        return self.git("diff", "--binary", "--full-index", "--no-color", "--no-ext-diff", "--no-textconv",
                        "--src-prefix=a/", "--dst-prefix=b/", old, new)

    def merge(self, base, ours, theirs):
        """The blob of a three-way merge of three blobs, or None when the changes overlap or a side is binary."""
        with tempfile.TemporaryDirectory(prefix="patched-sources-") as temporary, self.blobs() as read:
            paths = []
            for label, oid in (("ours", ours), ("base", base), ("theirs", theirs)):
                paths.append(str(Path(temporary) / label))
                Path(paths[-1]).write_bytes(read(oid))
            result = self.run(["git", "merge-file", "-p", *paths], ok=None)
        return self.text("hash-object", "-w", "--stdin", stdin=result.stdout) if result.returncode == 0 else None

    @contextlib.contextmanager
    def blobs(self):
        process = subprocess.Popen(self.command(["cat-file", "--batch"]), stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, env=git_environment())

        def read(oid):
            process.stdin.write(oid.encode("ascii") + b"\n")
            process.stdin.flush()
            header = process.stdout.readline().split()
            require(len(header) == 3 and header[1] == b"blob", f"object {oid} is not a blob in {self.path}")
            data = process.stdout.read(int(header[2]))
            process.stdout.read(1)
            return data
        try:
            yield read
        finally:
            process.stdin.close()
            process.stdout.close()
            process.wait()


def is_license(text):
    """Whether a text is an SPDX licence expression: ids joined by AND, OR and WITH, with parentheses."""
    tokens = re.findall(r"[()]|[^\s()]+", text)

    def identifier(at):
        return at < len(tokens) and tokens[at] not in ("AND", "OR", "WITH") and LICENSE_ID.fullmatch(tokens[at])

    def expression(at):
        """The index after one expression that starts at an index, or None."""
        while True:
            if tokens[at:at + 1] == ["("]:
                at = expression(at + 1)
                if at is None or tokens[at:at + 1] != [")"]:
                    return None
                at += 1
            elif identifier(at):
                at += 1
                if tokens[at:at + 1] == ["WITH"]:
                    if not identifier(at + 1):
                        return None
                    at += 2
            else:
                return None
            if tokens[at:at + 1] not in (["AND"], ["OR"]):
                return at
            at += 1

    return len(tokens) <= 64 and expression(0) == len(tokens)


def is_ref(value):
    """A tag or branch name. A commit id or a name under refs/ or pull/ is on no branch that upstream owns."""
    return bool(REF.fullmatch(value)) and ".." not in value and not OID.fullmatch(value) and not value.startswith(("refs/", "pull/"))


def is_url(value):
    return value.startswith(URL_SCHEMES) and not re.search(r"[\s\\\x00-\x1f\x7f]", value)


class Source:
    def __init__(self, root, name):
        self.name = name
        self.directory = Path(root) / PATCHES / name
        self.file = self.directory / "source.toml"
        require(self.file.is_file(), f"{name}: no {PATCHES}/{name}/source.toml")
        try:
            data = tomllib.loads(self.file.read_bytes().decode("utf-8"))
        except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
            raise Failure(f"{name}: source.toml: {error}") from error

        def check(condition, message):
            require(condition, f"{name}: source.toml: {message}")

        def text(table, key, test, shape, where=""):
            value = table.get(key)
            check(isinstance(value, str) and value, f"{where}{key} is needed")
            check(test(value), f"{where}{key} must be {shape}")
            return value

        def texts(key, default):
            value = data.get(key, default)
            check(isinstance(value, list) and all(isinstance(item, str) and item for item in value),
                  f"{key} must be a list of texts")
            return value

        url = f"a URL that starts with {' or '.join(URL_SCHEMES)}"
        check(not set(data) - KEYS, f"unknown keys: {', '.join(sorted(set(data) - KEYS))}")
        check(data.get("name") == name, f"name is {data.get('name')!r}")
        self.upstream = text(data, "upstream", is_url, url)
        self.commit = text(data, "commit", OID.fullmatch, "a full commit id")
        text(data, "ref", is_ref, "a tag or branch name")
        text(data, "origin", is_url, url)
        text(data, "origin_rev", OID.fullmatch, "a full commit id")
        text(data, "license", is_license, "an SPDX licence expression")
        text(data, "nix_hash", SRI.fullmatch, "an SRI hash: sha256-<base64>")
        self.paths, self.exclude, self.binary = texts("paths", ["."]), texts("exclude", []), texts("binary", [])
        check(self.paths, "paths must keep something")
        self.base_tree, self.tree = data.get("base_tree", ""), data.get("tree", "")
        for key, value in (("base_tree", self.base_tree), ("tree", self.tree)):
            check(isinstance(value, str) and (not value or OID.fullmatch(value)), f"{key} must be a full tree id")
        self.rev = data.get("rev", "")
        check(isinstance(self.rev, str) and (not self.rev or OID.fullmatch(self.rev)), "rev must be a full commit id")
        self.submodules = data.get("submodule", [])
        check(isinstance(self.submodules, list) and all(isinstance(item, dict) for item in self.submodules),
              "submodule must be [[submodule]] tables")
        for submodule in self.submodules:
            where = "[[submodule]] "
            check(not set(submodule) - SUBMODULE_KEYS, f"{where}unknown keys: {', '.join(sorted(set(submodule) - SUBMODULE_KEYS))}")
            text(submodule, "path", safe_path, "a path inside the tree", where)
            text(submodule, "url", is_url, url, where)
            text(submodule, "commit", OID.fullmatch, "a full commit id", where)
            text(submodule, "nix_hash", SRI.fullmatch, "an SRI hash: sha256-<base64>", where)

    @property
    def dest(self):
        return f"{EXPLORER}/bevy" if self.name == "bevy" else f"{EXPLORER}/forks/{self.name}"

    @property
    def pathspec(self):
        if self.paths == ["."] and not self.exclude:
            return []
        return [*(":(top)" + ("" if path == "." else path) for path in self.paths),
                *(f":(top,exclude){path}" for path in self.exclude)]

    def patches(self):
        series = self.directory / "series"
        return sorted(path for path in series.glob("*.patch") if path.is_file()) if series.is_dir() else []

    def inputs(self):
        return [("source.toml", self.file), *((f"series/{path.name}", path) for path in self.patches())]

    @property
    def ref(self):
        return f"refs/patched-sources/{self.name}/{self.rev}"


def names(root, selected=()):
    directory = Path(root) / PATCHES
    known = sorted(path.name for path in directory.iterdir() if (path / "source.toml").is_file()) if directory.is_dir() else []
    for name in selected:
        require(name in known, f"{name}: no {PATCHES}/{name}/source.toml")
    return list(selected) or known


def safe_path(path):
    parts = path.split("/")
    return (not any(part in ("", ".", "..") or part.lower() == ".git" for part in parts)
            and "\\" not in path and ":" not in parts[0])


def cache_directory():
    return Path(os.environ.get("DCL_PATCHED_SOURCES_CACHE") or Path.home() / ".cache/dcl-one/patched-sources")


@contextlib.contextmanager
def workspace(source, base_dir=None):
    """The source's repository, locked, and a scratch index. An exported base gets a repository that is discarded."""
    with tempfile.TemporaryDirectory(prefix="patched-sources-") as temporary:
        index = Path(temporary) / "index"
        if base_dir:
            yield Repo(Path(temporary) / "objects.git"), index
            return
        path = cache_directory() / f"{source.name}.git"
        with locked(path.with_name(path.name + ".lock"), source.name):
            yield Repo(path), index


def diff_of(patch):
    """The diff of a patch file: what follows the `---` line that ends its header."""
    at = patch.find(b"\n---\ndiff --git ")
    return patch[at + 5:] if at >= 0 else patch


def binary_patches(source):
    """The names of the patch files that hold a binary hunk, which no review can read."""
    return [patch.name for patch in source.patches() if BINARY_HUNK.search(diff_of(patch.read_bytes()))]


def build(source, repo, index, offline=False, base_dir=None):
    """Fill the index with the base under paths and exclude, then the series; (base tree, tree)."""
    binary = binary_patches(source)
    for name in binary:
        require(name in source.binary, f"{source.name}: series/{name} holds a binary hunk: list it under binary in source.toml")
    for name in source.binary:
        require(name in binary, f"{source.name}: binary lists {name}, which is no patch of the series with a binary hunk")
    if base_dir:
        directory = Path(base_dir) / source.name
        require(directory.is_dir(), f"{source.name}: no exported base at {directory}")
        links = [("160000", submodule["commit"], submodule["path"]) for submodule in source.submodules]
        files = walk(directory, skip={".git", *(submodule["path"] for submodule in source.submodules)})
        repo.load(index, [*repo.store(files), *links])
    else:
        repo.fetch(source.upstream, source.commit, offline, source.name)
        repo.git("read-tree", source.commit, index=index)
    if source.pathspec:
        repo.load(index, repo.rows(index, *source.pathspec))
    base_tree = repo.write_tree(index)
    for patch in source.patches():
        try:
            repo.git("apply", "--cached", "--whitespace=nowarn", "-", index=index, stdin=diff_of(patch.read_bytes()))
        except Failure as error:
            raise Failure(f"{source.name}: series/{patch.name} does not apply: {error}") from error
    # GitHub refuses a push of a commit with a workflow file from the token of a workflow, and from a token without the
    # workflow scope, so neither the CI job nor every contributor could publish the rev.
    workflows = repo.rows(index, ":(top).github/workflows")
    require(not workflows, f"{source.name}: the tree holds {workflows[0][2] if workflows else ''}: "
                           "add \".github\" to exclude in source.toml")
    return base_tree, repo.write_tree(index)


def differences(source, base_tree, tree):
    return [f"{source.name}: {key} is {built}, source.toml has {recorded or 'none'}"
            for key, built, recorded in (("base_tree", base_tree, source.base_tree), ("tree", tree, source.tree))
            if built != recorded]


def built(source, repo, index, offline=False, base_dir=None):
    base_tree, tree = build(source, repo, index, offline, base_dir)
    wrong = differences(source, base_tree, tree)
    require(not wrong, "; ".join(wrong))
    return tree


def submodule_rows(source, repo, submodule, offline, base_dir):
    exported = Path(base_dir) / source.name / submodule["path"] if base_dir else None
    files = walk(exported) if exported else []
    if files:
        return repo.store(files)
    label = f"{source.name}: submodule {submodule['path']}"
    repo.fetch(submodule["url"], submodule["commit"], offline, label)
    return [row for row in repo.tree_rows(submodule["commit"]) if row[0] != "160000"]


def resolve(entries, read, path):
    """What a link is written as: the file it names in the same tree, else its own target text."""
    text = entries[path][1]
    mode, oid = entries[path]
    seen = {path}
    while mode == "120000":
        path = posixpath.normpath(posixpath.join(posixpath.dirname(path), read(oid).decode("utf-8", "replace")))
        if path in seen or entries.get(path, ("160000", ""))[0] == "160000":
            return "100644", text
        seen.add(path)
        mode, oid = entries[path]
    return mode, oid


def layout(source, repo, index, offline=False, base_dir=None):
    """({path: (mode, blob id of the bytes on disk)}, submodule prefixes) of the dest for the tree in the index."""
    rows = {path: (mode, oid) for mode, oid, path in repo.rows(index)}
    groups = [("", rows)]
    for submodule in source.submodules:
        path = submodule["path"]
        require(rows.get(path) == ("160000", submodule["commit"]),
                f"{source.name}: the tree has {rows.get(path, ('', 'no gitlink'))[1]} at {path}, "
                f"[[submodule]] has {submodule['commit']}")
        groups.append((path + "/", {name: (mode, oid) for mode, oid, name in
                                    submodule_rows(source, repo, submodule, offline, base_dir)}))
    files = {}
    with repo.blobs() as read:
        for prefix, entries in groups:
            for path, (mode, oid) in entries.items():
                if mode == "160000":
                    continue
                name = prefix + path
                require(safe_path(name) and name.split("/")[0] not in LOCAL,
                        f"{source.name}: the tree has a path this tool cannot write: {name}")
                files[name] = resolve(entries, read, path) if mode == "120000" else (mode, oid)
    return files, tuple(prefix for prefix, _entries in groups[1:])


def commit_of(name, tree):
    """The commit a source is published as: the tree of its files as written, no parent, one fixed author and time."""
    return f"tree {tree}\nauthor {IDENT}\ncommitter {IDENT}\n\npatched source {name}\n".encode("utf-8")


def rev_of(name, tree):
    data = commit_of(name, tree)
    return hashlib.sha1(b"commit %d\0" % len(data) + data).hexdigest()


def write_commit(source, repo, index):
    """Write the commit of a source for the tree in the index, which is overwritten; (its id, the files of the dest)."""
    files = layout(source, repo, index)[0]
    tree = repo.tree_of(index, files)
    return repo.text("hash-object", "-t", "commit", "-w", "--stdin", stdin=commit_of(source.name, tree)), files


def read_stamp(dest):
    try:
        stamp = json.loads((dest / STAMP).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    if not isinstance(stamp, dict):
        return None
    for key, value in stamp.items():
        if key in ("executable", "local"):
            if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
                return None
        elif not isinstance(value, str):
            return None
    return stamp


def write_stamp(dest, **values):
    dest.mkdir(parents=True, exist_ok=True)
    write_atomic(dest / STAMP, (json.dumps(values, sort_keys=True) + "\n").encode("ascii"),
                 temporary=dest.with_name(f".{dest.name}.patched-stamp"))


def local_names(files):
    """Names in a dest that are not its content: Cargo writes a lock file where the tree has none."""
    return LOCAL | ({"Cargo.lock"} - set(files))


def record(source, repo, index, files):
    """Keep the tree of the files as written in the repository; the stamp of a dest that holds them."""
    written = repo.tree_of(index, files)
    repo.keep(written)
    digest = hashlib.sha256()
    with repo.blobs() as read:
        for name, (_mode, oid) in sorted(files.items()):
            data = read(oid)
            digest.update(name.encode("utf-8") + b"\0" + len(data).to_bytes(8, "little"))
            digest.update(data)
    return {"crc32": crc32(source.inputs()), "executable": sorted(name for name, (mode, _oid) in files.items() if mode == "100755"),
            "files": digest.hexdigest(), "local": sorted(local_names(files) - LOCAL), "tree": source.tree, "written": written}


def matches_inputs(source, stamp):
    return stamp.get("crc32") == crc32(source.inputs()) and stamp.get("tree") == source.tree


def stamp_local(stamp):
    return LOCAL | frozenset(stamp.get("local", ()))


def inspect(source, dest):
    """(missing | current | stale | edited, the stamp) of a dest."""
    stamp = read_stamp(dest)
    if stamp is None:
        return "missing", None
    files = walk(dest, stamp_local(stamp))
    # A dest that lost every file is written again: nothing in it is an edit to keep.
    if not stamp.get("files") or all(name == "Cargo.lock" for name, _path in files):
        return "stale", stamp
    # A stamp without the list is from a version that compared no modes; Windows has none to compare.
    if os.name != "nt" and stamp.get("executable", None) not in (
            None, [name for name, path in files if file_mode(path) == "100755"]):
        return "edited", stamp
    if stamp["files"] != sha256(files):
        return "edited", stamp
    return ("current" if matches_inputs(source, stamp) else "stale"), stamp


def written_rows(source, stamp):
    """{path: (mode, blob id)} a dest was written from, or None when the cache does not hold that tree."""
    path, tree = cache_directory() / f"{source.name}.git", stamp.get("written")
    if not tree or not (path / "HEAD").is_file():
        return None
    repo = Repo(path)
    return {name: (mode, oid) for mode, oid, name in repo.tree_rows(tree)} if repo.has(tree, "tree") else None


def edits_of(source, dest, stamp):
    """How an edited dest differs from what was written, in words."""
    rows = written_rows(source, stamp)
    if rows is None:
        return "not counted: the cache does not hold the tree it was written from"
    return counted(*compare(rows, scan(dest, stamp_local(stamp), rows)))


def write_files(repo, dest, files, keep=()):
    """Make dest hold exactly `files`, and the names to keep: only changed bytes are written; (written, removed)."""
    removed = 0
    for name, path in walk(dest, local_names(files)):
        if name not in files and name not in keep:
            path.unlink()
            removed += 1
    directories = []
    for directory, inside, _names in os.walk(dest):
        if Path(directory) == dest:
            inside[:] = sorted(set(inside) - LOCAL)
        else:
            directories.append(directory)
    for directory in reversed(directories):
        if not os.listdir(directory):
            os.rmdir(directory)
    written = 0
    with repo.blobs() as read:
        for name, (mode, oid) in sorted(files.items()):
            path = dest / name
            if path.is_symlink() or not path.is_file() or blob_id(path.read_bytes()) != oid:
                if path.is_symlink() or path.is_file():
                    path.unlink()
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(read(oid))
                require(blob_id(path.read_bytes()) == oid, f"{path} does not hold the bytes of blob {oid} after it was written")
                written += 1
            if os.name != "nt" and file_mode(path) != mode:
                path.chmod(0o755 if mode == "100755" else 0o644)
    return written, removed


def carry(source, repo, index, dest, stamp, files):
    """The edits of a dest merged into `files`, the new tree: (the files to write, the patch, lines to print).

    Each edited file is merged against the tree the dest was written from. The dest is only read.
    """
    name, old = source.name, stamp.get("written", "")
    require(old and repo.has(old, "tree"),
            f"{name}: {source.dest} was edited and the series changed, and the cache does not hold the tree it was written "
            f"from: restore the series it was written from and export the edits, or --force to drop them")
    base = {path: (mode, oid) for mode, oid, path in repo.tree_rows(old)}
    disk = scan(dest, stamp_local(stamp), base)
    changed, missing, added = compare(base, disk)
    edited = sorted([*changed, *missing, *added])
    for path in edited:
        require(disk.get(path, ("",))[0] != "120000",
                f"{name}: {path} is a link, which no rebuild carries over: replace it with a file, or --force to drop the edits")
    stored = repo.store([(path, dest / path) for path in (*changed, *added)], {path: base[path][0] for path in changed})
    theirs = {**base, **{path: (mode, oid) for mode, oid, path in stored}}
    for path in missing:
        del theirs[path]
    edits = repo.tree_of(index, theirs)
    patch = cache_directory() / "edits" / f"{name}-{edits[:12]}.patch"
    patch.parent.mkdir(parents=True, exist_ok=True)
    write_atomic(patch, repo.diff(old, edits), sync=True)
    lines = [f"{name}: the series changed: carrying {len(edited)} edited files over ({counted(changed, missing, added)})",
             f"{name}: edits saved as {patch}"]
    merged, conflicts = dict(files), []
    for path in edited:
        was, new, mine = base.get(path), files.get(path), theirs.get(path)
        if new == was:
            result, how = mine, "edit kept"
        elif new == mine:
            result, how = new, "edit already in the new series"
        elif None in (was, new, mine):
            conflicts.append(path)
            continue
        else:
            mode = mine[0] if new[0] == was[0] else new[0] if mine[0] in (was[0], new[0]) else None
            oid = (new[1] if mine[1] in (was[1], new[1]) else mine[1] if new[1] == was[1]
                   else repo.merge(was[1], new[1], mine[1]))
            if mode is None or oid is None:
                conflicts.append(path)
                continue
            result, how = (mode, oid), "edit merged with the new series"
        if result is None:
            merged.pop(path, None)
        else:
            merged[path] = result
        lines.append(f"{name}: {path}: {how}")
    if conflicts:
        print("\n".join(lines[:2]))
        raise Failure(f"{name}: the edits conflict with the new series in {', '.join(conflicts)}: "
                      f"{source.dest} is unchanged; --force drops the edits, and the saved patch can be applied by hand")
    return merged, patch, lines


def refresh(source, dest, force=False, offline=False, base_dir=None):
    """Write one dest when it is not current; True when it is. The caller holds its lock."""
    name = source.name
    condition, stamp = inspect(source, dest)
    if condition == "current":
        return True
    carried = condition == "edited" and not force
    if carried and matches_inputs(source, stamp):
        note(f"{name}: edited, kept ({edits_of(source, dest, stamp)})")
        return False
    if condition == "missing" and not force:
        require(not walk(dest, LOCAL | {"Cargo.lock"}), f"{name}: {source.dest} holds files and no stamp: --force replaces them")
    if condition == "stale" and stamp.get("edits"):
        note(f"{name}: a run that did not finish was carrying edits over; they are saved as {stamp['edits']}")
    with workspace(source, base_dir) as (repo, index):
        tree = built(source, repo, index, offline, base_dir)
        files, _inside = layout(source, repo, index, offline, base_dir)
        final = record(source, repo, index, files)
        rev = rev_of(name, final["written"])
        require(not source.rev or rev == source.rev, f"{name}: rev is {rev}, source.toml has {source.rev}")
        target, patch, lines = carry(source, repo, index, dest, stamp, files) if carried else (files, "", [])
        # An interrupted run leaves this stamp: stale to status and to the build, and safe to write over.
        write_stamp(dest, crc32="", files="", tree="", edits=str(patch))
        written, removed = write_files(repo, dest, target)
    write_stamp(dest, **final)
    for line in lines:
        print(line)
    print(f"{name}: {written} written, {removed} removed, tree {tree}")
    if lines:
        print(f"{name}: the edits are in {source.dest}: export turns them into a patch")
    return False


def lock_of(dest):
    return dest.with_name(f".{dest.name}.patched-lock")


def materialise(root, options):
    dest_root = Path(options.dest_root) if options.dest_root else Path(root)
    for name in names(root, options.names):
        source = Source(root, name)
        dest = dest_root / source.dest
        with locked(lock_of(dest), name):
            if refresh(source, dest, options.force, options.offline, options.base_dir):
                print(f"{name}: current")
    return 0


def unfilled(source, repo, index):
    """A line for each gitlink of the tree in the index that no [[submodule]] fills."""
    filled = {submodule["path"] for submodule in source.submodules}
    # An exported base holds no gitlink, so one that nothing fills must not be in the tree.
    return [f"{source.name}: gitlink {path} has no [[submodule]]: add one, or exclude the path"
            for mode, _oid, path in repo.rows(index) if mode == "160000" and path not in filled]


def verify(root, options):
    failed = 0
    for name in names(root, options.names):
        try:
            source = Source(root, name)
            with workspace(source) as (repo, index):
                base_tree, tree = build(source, repo, index)
                wrong = [*differences(source, base_tree, tree), *unfilled(source, repo, index)]
                if not wrong:
                    rev = write_commit(source, repo, index)[0]
                    if rev != source.rev:
                        wrong = [f"{name}: rev is {rev}, source.toml has {source.rev or 'none'}"]
        except Failure as error:
            wrong = [str(error)]
        for line in wrong:
            note(line)
        failed += bool(wrong)
        if not wrong:
            print(f"{name}: ok")
    return 1 if failed else 0


def status(root, options):
    for name in names(root, options.names):
        source = Source(root, name)
        print(f"{name}: {inspect(source, Path(root) / source.dest)[0]}")
    return 0


def slug(subject):
    return re.sub(r"[^a-z0-9]+", "-", subject.lower()).strip("-")[:60].rstrip("-") or "patch"


def patch_date():
    seconds = os.environ.get("SOURCE_DATE_EPOCH")
    try:
        moment = (datetime.datetime.fromtimestamp(int(seconds), datetime.timezone.utc) if seconds
                  else datetime.datetime.now(datetime.timezone.utc))
    except (ValueError, OverflowError, OSError) as error:
        raise Failure(f"SOURCE_DATE_EPOCH is no time in seconds: {seconds!r}") from error
    return moment.replace(microsecond=0).isoformat()


def is_junk(path):
    return any(fnmatch.fnmatchcase(posixpath.basename(path), pattern) for pattern in JUNK)


def with_tree(source, raw, tree, rev, binary, base_tree=None):
    """The bytes of source.toml with a new tree and rev, the base_tree when one is given and, when it grew, the
    binary list, in the file's aligned style."""
    updated = raw
    for key, value in (("base_tree", base_tree), ("tree", tree)):
        if value is not None:
            updated, count = re.subn(rb'(?m)^(%b\s*=\s*")[0-9a-f]*(")' % key.encode("ascii"),
                                     rb"\g<1>" + value.encode("ascii") + rb"\g<2>", updated)
            require(count == 1, f'{source.name}: source.toml needs exactly one line: {key} = "..."')
    updated, count = re.subn(rb'(?m)^(rev\s*=\s*")[0-9a-f]*(")', rb"\g<1>" + rev.encode("ascii") + rb"\g<2>", updated)
    if not count:
        line = f"{'rev':<10} = \"{rev}\"\n".encode("ascii")
        updated, count = re.subn(rb"(?m)^(tree\s*=[^\n]*)\n?", lambda match: match.group(1) + b"\n" + line, updated)
    require(count == 1, f'{source.name}: source.toml needs at most one line: rev = "..."')
    if binary == source.binary:
        return updated
    line = f"{'binary':<10} = [{', '.join(json.dumps(name) for name in binary)}]\n".encode("ascii")
    listed, count = re.subn(rb"(?ms)^binary\s*=\s*\[.*?\]\n", lambda _match: line, updated)
    if not count:
        listed, count = re.subn(rb"(?m)^(?=base_tree\s*=)", lambda _match: line, updated)
    require(count == 1, f"{source.name}: source.toml needs one base_tree line, which the binary list goes before")
    return listed


def export(root, options):
    name, subject = options.name, options.message.strip()
    require(subject and not re.search(r"[\x00-\x1f\x7f]", subject), "export: the subject is one line of text")
    names(root, [name])
    source = Source(root, name)
    dest = Path(root) / source.dest
    with locked(lock_of(dest), name):
        stamp = read_stamp(dest)
        require(stamp is not None and matches_inputs(source, stamp),
                f"{name}: {source.dest} was not written from the current series: materialise it before editing")
        number = max([int(path.name[:4]) for path in source.patches() if re.match(r"\d{4}-", path.name)], default=0) + 1
        require(number <= 9999, f"{name}: the series is full at patch 9999: fold patches into one before the next export")
        patch = source.directory / "series" / f"{number:04d}-{slug(subject)}.patch"
        header = f"From {ZERO}\nSubject: {subject}\nDate: {patch_date()}\n\n---\n".encode("utf-8")
        with workspace(source) as (repo, index):
            tree = built(source, repo, index)
            files, inside = layout(source, repo, index)
            changed, missing, added = compare(files, scan(dest, local_names(files), files))
            junk = [path for path in added if is_junk(path)]
            for path in junk:
                note(f"{name}: skipped {path}: it is left in {source.dest} and is not in the patch")
            edited = [path for path in (*changed, *added) if path not in junk]
            for path in (*missing, *edited):
                require(not path.startswith(inside), f"{name}: {path} is inside a submodule; its edits cannot be exported")
            # The index holds a link where the dest holds its copy; an edited copy replaces the link with a file.
            modes = {path: mode for mode, _oid, path in repo.rows(index)}
            repo.update(index, [*repo.store([(path, dest / path) for path in edited], {path: files[path][0] for path in changed}),
                                *((modes[path], ZERO, path) for path in missing)])
            new_tree = repo.write_tree(index)
            require(new_tree != tree, f"{name}: {source.dest} has no edits")
            diff = repo.diff(tree, new_tree)
            binary = [*source.binary, patch.name] if BINARY_HUNK.search(diff) else source.binary
            require(binary == source.binary or options.binary,
                    f"{name}: the edits change binary files, which no review can read: "
                    f"--binary lists {patch.name} under binary in source.toml")
            repo.git("read-tree", tree, index=index)
            repo.git("apply", "--cached", "--whitespace=nowarn", "-", index=index, stdin=diff)
            require(repo.write_tree(index) == new_tree, f"{name}: the new patch does not reproduce the dest")
            # Copies of links follow the files they name, so the dest is again what materialise writes.
            rev, files = write_commit(source, repo, index)
            updated = with_tree(source, source.file.read_bytes(), new_tree, rev, binary)
            with explorer(root) as (cargo, texts):
                named = as_git(source, rev, *texts) if cargo else ()
                patch.parent.mkdir(exist_ok=True)
                write_atomic(patch, header + diff, sync=True)
                try:
                    write_atomic(source.file, updated, sync=True)
                except BaseException:
                    patch.unlink()
                    raise
                try:
                    write_texts(cargo, texts, named)
                except BaseException:
                    note(f"{name}: source.toml has rev {rev}, and Cargo.toml or Cargo.lock does not: `repin {name}` writes it there")
                    raise
            source = Source(root, name)
            final = record(source, repo, index, files)
            write_files(repo, dest, files, keep=junk)
        write_stamp(dest, **final)
    print(f"{name}: {patch.relative_to(Path(root)).as_posix()}, tree {new_tree}, rev {rev}")
    return 0


def parsed(text, label):
    try:
        return tomllib.loads(text)
    except tomllib.TOMLDecodeError as error:
        raise Failure(f"{label}: {error}") from error


def cargo_texts(root):
    """(Cargo.toml, Cargo.lock) of the Explorer and their texts; nothing when the root has no such manifest."""
    files = (Path(root) / WORKSPACE / "Cargo.toml", Path(root) / WORKSPACE / "Cargo.lock")
    if not files[0].is_file():
        return (), ()
    require(files[1].is_file(), f"no {WORKSPACE}/Cargo.lock")
    return files, tuple(file.read_bytes().decode("utf-8") for file in files)


@contextlib.contextmanager
def explorer(root):
    """The Explorer's two Cargo files and their texts, locked when the root has them."""
    with contextlib.ExitStack() as stack:
        if (Path(root) / WORKSPACE / "Cargo.toml").is_file():
            stack.enter_context(locked(lock_of(Path(root) / WORKSPACE), "Cargo.toml"))
        yield cargo_texts(root)


def write_texts(files, old, new):
    for file, before, after in zip(files, old, new):
        if after != before:
            write_atomic(file, after.encode("utf-8"), sync=True)


def entries_of(manifest):
    """(key, table) of each table of a manifest that names a path or a git repository, wherever it is."""
    found, pending = [], [("", manifest)]
    while pending:
        key, table = pending.pop()
        if isinstance(table.get("path"), str) or isinstance(table.get("git"), str):
            found.append((key, table))
        pending.extend((name, value) for name, value in table.items() if isinstance(value, dict))
    return sorted(found, key=lambda entry: entry[0])


def crate_of_entry(key, table):
    return table["package"] if isinstance(table.get("package"), str) else key


def dest_of(path):
    """The dest that a path of the Explorer manifest is in, or None."""
    parts = posixpath.normpath(posixpath.join(WORKSPACE, path.replace("\\", "/"))).split("/")
    depth = EXPLORER.count("/") + 1
    if "/".join(parts[:depth]) != EXPLORER or parts[depth:depth + 1] not in (["bevy"], ["forks"]):
        return None
    return "/".join(parts[:depth + (1 if parts[depth] == "bevy" else 2)])


def path_entries(manifest, dest):
    """The keys of the entries of a parsed manifest that read a dest by path."""
    return [key for key, table in entries_of(manifest) if isinstance(table.get("path"), str) and dest_of(table["path"]) == dest]


def rev_entries(manifest, rev):
    return [key for key, table in entries_of(manifest) if table.get("git") == MIRROR and table.get("rev") == rev]


def lock_blocks(text):
    """A lock file as blocks, each a list of lines. Blank lines part them, and `lock_text` gives the text back."""
    return [block.split("\n") for block in text.split("\n\n")]


def lock_text(blocks):
    return "\n\n".join("\n".join(block) for block in blocks)


def package_of(block):
    """(name, version, source or "") of a package block of a lock file, (None, None, "") of another block."""
    if block[0] not in ("[[package]]", "[[patch.unused]]"):
        return None, None, ""
    fields = dict(match.groups() for match in map(LOCK_FIELD.fullmatch, block[1:4]) if match)
    require("name" in fields and "version" in fields, "Cargo.lock: a package has no name or no version")
    return fields["name"], fields["version"], fields.get("source", "")


def dependency(line):
    """(name, version or None, source or "") that a dependency line of a lock file names."""
    match = LOCK_DEPENDENCY.fullmatch(line)
    require(match, f"Cargo.lock: this is no dependency: {line.strip()}")
    return match.group(1), match.group(2), match.group(3) or ""


def dependencies_of(block):
    """(start, end) of the dependency lines of a block."""
    if "dependencies = [" not in block:
        return 0, 0
    start = block.index("dependencies = [") + 1
    return start, block.index("]", start)


def source_order(source):
    """Where Cargo puts a source among packages of one name and version: a path, then a registry, then git."""
    if not source.startswith("git+"):
        return (1, source) if source else (0,)
    url, _mark, reference = source[4:].partition("#")[0].partition("?")
    kind, _mark, value = reference.partition("=")
    return 2, {"tag": 0, "branch": 1, "rev": 2}.get(kind, 3), value, url


def in_order(items, identity, moved):
    """Items as Cargo writes them: a run of one name and version in `moved` is sorted again, by source."""
    result = []
    for same, run in itertools.groupby(items, lambda item: identity(item)[:2]):
        run = list(run)
        result.extend(sorted(run, key=lambda item: source_order(identity(item)[2])) if same in moved else run)
    return result


def lock_as_path(text, rev):
    """A lock file with the packages of a rev as path packages: no source, and none where another package names them."""
    source = f"git+{MIRROR}?rev={rev}"
    line, suffix = f'source = "{source}#{rev}"', f' ({source})",'
    blocks, moved = lock_blocks(text), set()
    for block in blocks:
        if line in block[:4]:
            block.remove(line)
            moved.add(package_of(block)[:2])
        start, end = dependencies_of(block)
        named = set()
        for at in range(start, end):
            if block[at].endswith(suffix):
                block[at] = block[at][:-len(suffix)] + '",'
                named.add(dependency(block[at])[:2])
        block[start:end] = in_order(block[start:end], dependency, named)
    return lock_text(in_order(blocks, package_of, moved))


def lock_as_git(text, rev, roots, barrier, label):
    """A lock file with the path packages of a source as packages of a rev.

    They are the packages named in `roots` and what those need, as far as a package with a source or
    one named in `barrier`, which another source holds.
    """
    source = f"git+{MIRROR}?rev={rev}"
    blocks = lock_blocks(text)
    packages = [package_of(block) for block in blocks]
    copies = collections.Counter(package[:2] for package in packages)
    local = {}
    for at, (name, _version, origin) in enumerate(packages):
        if name and not origin:
            local.setdefault(name, []).append(at)

    def path_package(name, version=None):
        found = [at for at in local.get(name, ()) if version in (None, packages[at][1])]
        require(len(found) < 2, f"{label}: Cargo.lock has {len(found)} packages {name} without a source")
        return found[0] if found else None

    chosen, pending = set(), []
    for name in roots:
        at = path_package(name)
        require(at is not None or (name, f"{source}#{rev}") in {(package[0], package[2]) for package in packages},
                f"{label}: Cargo.lock has no package {name} without a source")
        pending.append(at)
    while pending:
        at = pending.pop()
        if at is None or at in chosen:
            continue
        chosen.add(at)
        start, end = dependencies_of(blocks[at])
        for name, version, origin in map(dependency, blocks[at][start:end]):
            if not origin and name not in barrier:
                pending.append(path_package(name, version))
    moved = {packages[at][:2] for at in chosen}
    for at in chosen:
        blocks[at].insert(3, f'source = "{source}#{rev}"')
    for block in blocks:
        start, end = dependencies_of(block)
        named = set()
        for at in range(start, end):
            name, version, origin = dependency(block[at])
            # Cargo names the source only where two packages share the name and the version.
            if version and not origin and (name, version) in moved and copies[name, version] > 1:
                block[at] = f' "{name} {version} ({source})",'
                named.add((name, version))
        block[start:end] = in_order(block[start:end], dependency, named)
    return lock_text(in_order(blocks, package_of, moved))


def package_name(data):
    """The name of the package of a manifest, or None."""
    try:
        package = tomllib.loads(data.decode("utf-8")).get("package")
    except (UnicodeDecodeError, tomllib.TOMLDecodeError):
        return None
    return package["name"] if isinstance(package, dict) and isinstance(package.get("name"), str) else None


def crate_directories(dest):
    """{package name: [directory in the dest]} of the manifests of a dest."""
    found = {}
    for name, file in walk(dest, LOCAL):
        package = package_name(file.read_bytes()) if posixpath.basename(name) == "Cargo.toml" else None
        if package:
            found.setdefault(package, []).append(posixpath.dirname(name))
    return found


def crates_of(repo, files):
    """The names of the packages of a dest that holds these files."""
    with repo.blobs() as read:
        found = {package_name(read(oid)) for path, (_mode, oid) in files.items() if posixpath.basename(path) == "Cargo.toml"}
    return found - {None}


def require_one_line(name, keys, what):
    """Fail on the entries that a rewrite of the manifest left as they were."""
    if keys:
        raise Failure(f'{name}: Cargo.toml {what} for {keys[0]} in a form this tool does not change: '
                      f'write it on one line: {keys[0]} = {{ git = "{MIRROR}", rev = "..." }}')


def as_path(source, dest, manifest, lock):
    """The texts of Cargo.toml and Cargo.lock with a source read from its dest instead of its rev."""
    name, rev = source.name, source.rev
    tables = dict(entries_of(parsed(manifest, "Cargo.toml")))
    directories = crate_directories(dest)

    def path_of(match):
        key = match.group(1)
        crate = crate_of_entry(key, tables.get(key, {}))
        found = directories.get(crate, [])
        require(len(found) == 1, f"{name}: {source.dest} has {len(found)} packages named {crate}, and {key} needs one")
        directory = posixpath.normpath(posixpath.join(source.dest, found[0]))
        return f'{key}{match.group(2)}path = "{posixpath.relpath(directory, WORKSPACE)}"'

    entry = rf'(?m)^([A-Za-z0-9_-]+)(\s*=\s*\{{[^\n]*?(?<![A-Za-z0-9_-]))git\s*=\s*"{re.escape(MIRROR)}"\s*,\s*rev\s*=\s*"{rev}"'
    manifest = re.sub(entry, path_of, manifest)
    after = parsed(manifest, "Cargo.toml")
    require_one_line(name, rev_entries(after, rev), "names the rev")
    require(path_entries(after, source.dest), f"{name}: Cargo.toml has no entry with rev {rev}")
    return manifest, lock_as_path(lock, rev)


def as_git(source, rev, manifest, lock, old=None):
    """The texts of Cargo.toml and Cargo.lock with a source taken at a rev.

    The entries that read its dest by path name the rev, and the packages they reach in the lock file
    get it as their source. Where the files name `old`, the rev the source had before, they name the new one.
    """
    old = source.rev if old is None else old
    before = parsed(manifest, "Cargo.toml")
    reads = [(crate_of_entry(key, table), dest_of(table["path"])) for key, table in entries_of(before)
             if isinstance(table.get("path"), str)]
    roots = sorted({crate for crate, dest in reads if dest == source.dest})
    barrier = {crate for crate, dest in reads if dest not in (None, source.dest)}

    def git_of(match):
        if dest_of(match.group(3)) != source.dest:
            return match.group(0)
        return f'{match.group(1)}{match.group(2)}git = "{MIRROR}", rev = "{rev}"'

    manifest = re.sub(r'(?m)^([A-Za-z0-9_-]+)(\s*=\s*\{[^\n]*?(?<![A-Za-z0-9_-]))path\s*=\s*"([^"\n]*)"', git_of, manifest)
    require_one_line(source.name, path_entries(parsed(manifest, "Cargo.toml"), source.dest), f"reads {source.dest}")
    at = rev
    if not roots and old:
        # A run that wrote Cargo.toml and stopped before Cargo.lock left the packages of the rev without a source.
        packages = [package_of(block) for block in lock_blocks(lock)]
        held = {crate for crate, _version, origin in packages if origin == f"git+{MIRROR}?rev={old}#{old}"}
        local = {crate for crate, _version, origin in packages if crate and not origin}
        named = {crate_of_entry(key, table) for key, table in entries_of(before)
                 if table.get("git") == MIRROR and table.get("rev") == old}
        roots, at = sorted((named & local) - held), old
    if roots:
        lock = lock_as_git(lock, at, roots, barrier, source.name)
    if old and old != rev:
        manifest, lock = manifest.replace(old, rev), lock.replace(old, rev)
    return manifest, lock


def package_rev(source):
    """The rev at which a source line of the lock file takes a package from MIRROR, or None."""
    match = re.fullmatch(re.escape(f"git+{MIRROR}") + r"\?rev=([0-9a-f]{40})#\1", source)
    return match.group(1) if match else None


def pinned(root, source, rev, crates, manifest, lock):
    """The texts of Cargo.toml and Cargo.lock with a source taken at a rev, whatever they name for it now.

    That is its dest, the rev of source.toml, or a rev that no source has: a run that stopped between its
    writes leaves the rev that source.toml had before. Such a rev is one that only crates of the source have.
    """
    others = {Source(root, name).rev for name in names(root) if name != source.name}
    # A rev line that is a copy of another source's is not a rev of this one: the files name it for the other.
    old = "" if source.rev in others else source.rev
    known = {rev, old, *others}
    named = [(table.get("rev"), crate_of_entry(key, table)) for key, table in entries_of(parsed(manifest, "Cargo.toml"))
             if table.get("git") == MIRROR]
    named.extend((package_rev(origin), crate) for crate, _version, origin in map(package_of, lock_blocks(lock)))
    found = {}
    for other, crate in named:
        if isinstance(other, str) and OID.fullmatch(other):
            found.setdefault(other, set()).add(crate)
    for stray in sorted(other for other, held in found.items() if other not in known and held <= crates):
        manifest, lock = manifest.replace(stray, rev), lock.replace(stray, rev)
    return as_git(source, rev, manifest, lock, old)


def repin(root, options):
    for name in names(root, options.names):
        dest = Path(root) / Source(root, name).dest
        with locked(lock_of(dest), name):
            source = Source(root, name)
            with workspace(source) as (repo, index):
                base_tree, tree = build(source, repo, index)
                wrong = unfilled(source, repo, index)
                require(not wrong, "; ".join(wrong))
                rev, files = write_commit(source, repo, index)
                crates = crates_of(repo, files)
            raw = source.file.read_bytes()
            updated = with_tree(source, raw, tree, rev, source.binary, base_tree)
            with explorer(root) as (cargo, texts):
                new = pinned(root, source, rev, crates, *texts) if cargo else ()
                if updated != raw:
                    write_atomic(source.file, updated, sync=True)
                write_texts(cargo, texts, new)
        print(f"{name}: tree {tree}, rev {rev}")
        if base_tree != source.base_tree:
            note(f"{name}: the base changed: nix_hash must be the hash of commit {source.commit}, and this tool does not write it")
        if cargo:
            entry = f'{{ git = "{MIRROR}", rev = "{rev}" }}'
            require(rev_entries(parsed(new[0], "Cargo.toml"), rev),
                    f"{name}: Cargo.toml has no entry of this source: write one, on one line, as {entry}")
            require(any(package_rev(origin) == rev for _crate, _version, origin in map(package_of, lock_blocks(new[1]))),
                    f"{name}: Cargo.lock has no package of this source: write its source as git+{MIRROR}?rev={rev}#{rev}")
    return 0


def edit(root, options):
    name = options.name
    names(root, [name])
    source = Source(root, name)
    require(source.rev, f"{name}: source.toml has no rev: verify prints it")
    dest = Path(root) / source.dest
    if not options.stop:
        with locked(lock_of(dest), name):
            refresh(source, dest)
    with explorer(root) as (cargo, texts):
        require(cargo, f"no {WORKSPACE}/Cargo.toml")
        new = (pinned(root, source, source.rev, set(crate_directories(dest)), *texts) if options.stop
               else as_path(source, dest, *texts))
        require(not options.stop or rev_entries(parsed(new[0], "Cargo.toml"), source.rev),
                f"{name}: Cargo.toml has no entry that reads {source.dest}")
        write_texts(cargo, texts, new)
    if options.stop and inspect(source, dest)[0] == "edited":
        note(f"{name}: {source.dest} is edited: the edits stay there, and the Explorer no longer builds them")
    print(f"{name}: Cargo.toml and Cargo.lock {'already ' if new == texts else ''}"
          + (f"name rev {source.rev}" if options.stop else f"read {source.dest}"))
    return 0


def canonical(url):
    """A git URL without what GitHub does not read: the case of the letters, `.git` and `/` at the end."""
    url = url.lower()
    while url.endswith(("/", ".git")):
        url = url.removesuffix("/").removesuffix(".git")
    return url


def pins(root, _options):
    sources = [Source(root, name) for name in names(root)]
    cargo, texts = cargo_texts(root)
    require(cargo, f"no {WORKSPACE}/Cargo.toml")
    manifest, lock = parsed(texts[0], "Cargo.toml"), parsed(texts[1], "Cargo.lock")
    revs = {source.rev for source in sources}
    wrong = [f"{source.name}: source.toml has no rev" for source in sources if not source.rev]
    crates, named, paths, others = set(), {"Cargo.toml": set(), "Cargo.lock": set()}, {}, []
    for key, table in entries_of(manifest):
        crate = crate_of_entry(key, table)
        if table.get("git") == MIRROR:
            crates.add(crate)
            rev = table.get("rev") if isinstance(table.get("rev"), str) else ""
            named["Cargo.toml"].add(rev)
            if not rev or rev not in revs:
                wrong.append(f"Cargo.toml: {key}: rev {rev or 'none'} is the rev of no source")
        elif "git" in table:
            others.append(("Cargo.toml", key, crate, str(table["git"]), str(table["git"])))
        elif dest_of(table["path"]):
            paths.setdefault(dest_of(table["path"]), []).append(key)
    packages = [*lock.get("package", []), *lock.get("patch", {}).get("unused", [])]
    for package in packages:
        crate, source = package.get("name", ""), package.get("source", "")
        what = f"{crate} {package.get('version', '')}"
        url = source[4:].partition("#")[0].partition("?")[0]
        if source.startswith("git+") and url == MIRROR:
            crates.add(crate)
            named["Cargo.lock"].add(package_rev(source) or "")
            if package_rev(source) not in revs:
                wrong.append(f"Cargo.lock: {what}: source {source} names the rev of no source")
        elif source.startswith("git+"):
            others.append(("Cargo.lock", what, crate, url, source))
    for file, what, crate, url, shown in others:
        # Another spelling of MIRROR names a commit that nothing above compares with the rev of a source.
        if canonical(url) == canonical(MIRROR):
            wrong.append(f"{file}: {what}: {shown} is another spelling of {MIRROR}, and only that text is accepted")
        elif url not in OTHER_GIT:
            wrong.append(f"{file}: {what}: {shown} is not {MIRROR} and not in OTHER_GIT, the other git sources that this tool accepts")
        elif crate in crates:
            wrong.append(f"{file}: {what}: {crate} comes from {shown}, and other entries take it from {MIRROR}")
        elif file == "Cargo.lock" and shown.partition("#")[2] != OTHER_GIT[url]:
            wrong.append(f"{file}: {what}: {shown} is not at commit {OTHER_GIT[url]}, the one that OTHER_GIT gives for {url}")
    for dest, keys in sorted(paths.items()):
        source = next((source for source in sources if source.dest == dest), None)
        end = f"`edit --stop {source.name}` names the rev again" if source else "no source has this dest"
        wrong.append(f"{source.name if source else 'Cargo.toml'}: Cargo.toml reads {dest} by path ({', '.join(sorted(set(keys)))}): {end}")
    # A source that Cargo.toml reads by path is in neither file; one message says so.
    wrong.extend(f"{source.name}: rev {source.rev} is not in {file}" for source in sources for file in named
                 if source.rev and source.dest not in paths and source.rev not in named[file])
    # A key that two tables of Cargo.toml hold is one problem.
    for line in dict.fromkeys(wrong):
        note(line)
    if not wrong:
        print(f"pins: {len(sources)} revs in Cargo.toml and Cargo.lock")
    return 1 if wrong else 0


def served(remote, shown, rev):
    """Whether the remote serves the commit rev by id, with or without a ref: Cargo fetches a rev that way.

    Only the answer `not our ref` means that it does not: any other failure is no answer, and fails the run.
    """
    with tempfile.TemporaryDirectory(prefix="patched-sources-") as directory:
        repo = Repo(directory)
        # The commit alone, without its tree.
        fetch = repo.command(["fetch", "-q", "--no-tags", "--depth", "1", "--filter=tree:0", "--end-of-options", remote, rev])
        try:
            result = Repo.run(fetch, environment=remote_environment(), timeout=PROBE_SECONDS, ok=None)
        except Failure as error:
            raise Failure(f"publish: {shown}: cannot check {rev}: {error}") from error
        if result.returncode == 0:
            return repo.has(rev, "commit")
        error = result.stderr.decode("utf-8", "replace")
        if "not our ref" in error:
            return False
        lines = [line for line in error.replace(remote, shown).strip().splitlines() if not line.startswith(("hint:", "warning:"))]
        raise Failure(f"publish: {shown}: cannot check {rev}: {'; '.join(lines[-3:]) or f'git exit {result.returncode}'}")


def publish(root, options):
    require(options.check or not options.fetchable, "publish: --fetchable needs --check")
    sources = [Source(root, name) for name in names(root, options.names)]
    for source in sources:
        require(source.rev, f"{source.name}: source.toml has no rev: verify prints it")
    remote = options.remote
    shown = re.sub(r"(?<=://)[^/@]*@", "", remote)

    def git(command, timeout):
        try:
            return Repo.run(command, environment=remote_environment(), timeout=timeout).stdout
        except Failure as error:
            raise Failure(f"publish: {shown}: {str(error).replace(remote, shown)}") from error

    # Nothing after --end-of-options is read as an option, whatever the remote is.
    listing = git(["git", *Repo.OPTIONS, "ls-remote", "--refs", "--end-of-options", remote, "refs/patched-sources/*"],
                  FETCH_SECONDS).decode("utf-8", "replace")
    there = {ref: oid for oid, _tab, ref in (line.partition("\t") for line in listing.splitlines())}
    for source in sources:
        require(there.get(source.ref, source.rev) == source.rev,
                f"{source.name}: {shown} has {source.ref} at {there.get(source.ref)}: remove that ref by hand")
    missing = 0
    for source in sources:
        name, present = source.name, source.ref in there
        if not present and options.fetchable and served(remote, shown, source.rev):
            print(f"{name}: fetchable, no ref")
            continue
        missing += not present
        if present or options.check:
            print(f"{name}: {'published' if present else 'missing ' + source.ref}")
            continue
        with workspace(source) as (repo, index):
            # What is pushed is built here, so a rev that names another commit of the cache is never published.
            built(source, repo, index)
            rev = write_commit(source, repo, index)[0]
            require(rev == source.rev, f"{name}: rev is {rev}, source.toml has {source.rev}")
            repo.git("update-ref", source.ref, source.rev)
            git(repo.command(["push", "-q", "--end-of-options", remote, f"{source.rev}:{source.ref}"]), PUSH_SECONDS)
        print(f"{name}: pushed {source.ref}")
    return 1 if options.check and missing else 0


def crate_of(path, crates):
    directory = posixpath.dirname(path)
    while directory and directory not in crates:
        directory = posixpath.dirname(directory)
    return directory or ("." if "" in crates else "(no crate)")


def divergence(root, options):
    for name in names(root, options.names):
        source = Source(root, name)
        with workspace(source) as (repo, index):
            base_tree, tree = build(source, repo, index)
            crates = {posixpath.dirname(path) for _mode, _oid, path in repo.rows(index)
                      if posixpath.basename(path) == "Cargo.toml"}
            numstat = repo.git("diff", "--numstat", "--no-renames", "-z", base_tree, tree)
        total, by_crate = [0, 0, 0], {}
        for entry in filter(None, numstat.split(b"\0")):
            added, deleted, path = entry.decode("utf-8").split("\t", 2)
            for counts in (total, by_crate.setdefault(crate_of(path, crates), [0, 0, 0])):
                counts[0] += 1
                counts[1] += 0 if added == "-" else int(added)
                counts[2] += 0 if deleted == "-" else int(deleted)
        print(f"{name}: {len(source.patches())} patches, {total[0]} files, +{total[1]} -{total[2]}")
        for crate, (files, added, deleted) in sorted(by_crate.items(), key=lambda item: (-item[1][0], item[0])):
            print(f"  {crate}: {files} files, +{added} -{deleted}")
    return 0


def main(arguments=None, root=ROOT):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--root", metavar="DIR", help="the checkout to work on; the one that holds this tool when not given")
    commands = parser.add_subparsers(dest="command", required=True)
    command = commands.add_parser("materialise", help="write each dest from its base and series")
    command.add_argument("names", nargs="*", metavar="NAME")
    command.add_argument("--offline", action="store_true", help="never fetch")
    command.add_argument("--base-dir", metavar="DIR", help="read exported bases from DIR/<name>/")
    command.add_argument("--dest-root", metavar="DIR", help="write below DIR instead of the repository root")
    command.add_argument("--force", action="store_true", help="replace a dest that was edited")
    command.set_defaults(run=materialise)
    for name, run, text in (("verify", verify, "check that base_tree, tree and rev reproduce"),
                            ("status", status, "missing, current, stale or edited"),
                            ("divergence", divergence, "files and lines changed against the base, by crate")):
        command = commands.add_parser(name, help=text)
        command.add_argument("names", nargs="*", metavar="NAME")
        command.set_defaults(run=run)
    command = commands.add_parser("export", help="turn the edits in a dest into the next patch")
    command.add_argument("name", metavar="NAME")
    command.add_argument("-m", "--message", required=True, metavar="SUBJECT")
    command.add_argument("--binary", action="store_true", help="accept edits to binary files and list the patch under binary")
    command.set_defaults(run=export)
    command = commands.add_parser("repin", help="write base_tree, tree and rev again, and the rev in the Explorer's Cargo files")
    command.add_argument("names", nargs="+", metavar="NAME")
    command.set_defaults(run=repin)
    command = commands.add_parser("pins", help="check that the Explorer's Cargo files name the rev of every source")
    command.set_defaults(run=pins)
    command = commands.add_parser("publish", help="push the commit of each source that the remote lacks")
    command.add_argument("names", nargs="*", metavar="NAME")
    command.add_argument("--remote", default=MIRROR, metavar="URL",
                         help=f"another repository to push to, for a trial; {MIRROR} when not given")
    command.add_argument("--check", action="store_true", help="push nothing: list what the remote lacks, exit 1 if anything")
    command.add_argument("--fetchable", action="store_true",
                         help="with --check: a rev that the remote serves by id, without a ref, is not lacking")
    command.set_defaults(run=publish)
    command = commands.add_parser("edit", help="make the Explorer build one source from its dest")
    command.add_argument("name", metavar="NAME")
    command.add_argument("--stop", action="store_true", help="name the rev again in the Explorer's Cargo files")
    command.set_defaults(run=edit)
    options = parser.parse_args(arguments)
    try:
        if options.root:
            root = Path(options.root).resolve()
            require((root / PATCHES).is_dir(), f"--root {options.root}: no {PATCHES}")
        return options.run(root, options)
    except (Failure, OSError, UnicodeError) as error:
        note(error)
        return 1
    except KeyboardInterrupt:
        note("interrupted")
        return 130


if __name__ == "__main__":
    sys.exit(main())
