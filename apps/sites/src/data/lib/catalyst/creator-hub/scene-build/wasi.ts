// The WASI preview1 host the scene-build wasm (library/dcl-scene-build)
// runs on: an in-memory file tree preopened at "/", stdout/stderr captured as
// text, no sockets, no threads. It implements exactly the imports that module
// declares (a missing one fails instantiation by name, which is the signal to
// extend this file). Errno, struct layouts and flag bits follow
// wasi_snapshot_preview1's witx.

const ERRNO = {
  SUCCESS: 0,
  ACCES: 2,
  BADF: 8,
  EXIST: 20,
  INVAL: 28,
  IO: 29,
  ISDIR: 31,
  NOENT: 44,
  NOSYS: 52,
  NOTDIR: 54,
  NOTEMPTY: 55,
} as const;

const FILETYPE_CHARACTER_DEVICE = 2;
const FILETYPE_DIRECTORY = 3;
const FILETYPE_REGULAR_FILE = 4;

const OFLAGS_CREAT = 1;
const OFLAGS_DIRECTORY = 2;
const OFLAGS_EXCL = 4;
const OFLAGS_TRUNC = 8;
const FDFLAGS_APPEND = 1;

const ALL_RIGHTS = 0x1fffffffn;

type FileNode = { kind: "file"; ino: bigint; data: Uint8Array; size: number; mtime: bigint };
type DirNode = { kind: "dir"; ino: bigint; entries: Map<string, Node>; mtime: bigint };
type Node = FileNode | DirNode;

/** A directory tree held in memory; paths are "/"-separated and relative to its root. */
export class MemFs {
  private nextIno = 1n;
  readonly root: DirNode = this.dir();

  private dir(): DirNode {
    return { kind: "dir", ino: this.nextIno++, entries: new Map(), mtime: now() };
  }

  newFile(data: Uint8Array = new Uint8Array(0)): FileNode {
    return { kind: "file", ino: this.nextIno++, data, size: data.byteLength, mtime: now() };
  }

  newDir(): DirNode {
    return this.dir();
  }

  /** Write `data` at `path`, creating parent directories. The bytes are shared, not copied. */
  writeFile(path: string, data: Uint8Array): void {
    const parts = split(path);
    const name = parts.pop();
    if (!name) throw new Error(`not a file path: ${path}`);
    let cur = this.root;
    for (const part of parts) {
      let next = cur.entries.get(part);
      if (!next) {
        next = this.dir();
        cur.entries.set(part, next);
      }
      if (next.kind !== "dir") throw new Error(`${path}: ${part} is a file`);
      cur = next;
    }
    cur.entries.set(name, this.newFile(data));
  }

  /** The bytes at `path`, or null when it is missing or a directory. */
  readFile(path: string): Uint8Array | null {
    const node = this.lookup(this.root, split(path));
    return node?.kind === "file" ? node.data.subarray(0, node.size) : null;
  }

  lookup(from: DirNode, parts: string[]): Node | null {
    let cur: Node = from;
    for (const part of parts) {
      if (cur.kind !== "dir") return null;
      const next: Node | undefined = cur.entries.get(part);
      if (!next) return null;
      cur = next;
    }
    return cur;
  }
}

function split(path: string): string[] {
  return path.split("/").filter((p) => p !== "" && p !== ".");
}

function now(): bigint {
  return BigInt(Date.now()) * 1_000_000n;
}

/** Thrown out of `proc_exit` to unwind `_start`. */
export class WasiExit extends Error {
  constructor(readonly code: number) {
    super(`exit ${code}`);
  }
}

type Open =
  | { kind: "stdio"; fd: number }
  | { kind: "dir"; node: DirNode; preopen?: string }
  | { kind: "file"; node: FileNode; offset: number; append: boolean };

export type WasiOptions = { args: string[]; env?: Record<string, string>; fs: MemFs };

export class Wasi {
  private readonly fds = new Map<number, Open>();
  private nextFd = 4;
  private memory: WebAssembly.Memory | null = null;
  private readonly out: Uint8Array[] = [];
  private readonly err: Uint8Array[] = [];
  private readonly args: Uint8Array[];
  private readonly env: Uint8Array[];
  readonly imports: Record<string, (...a: never[]) => number | void>;

  constructor(private readonly opts: WasiOptions) {
    const enc = new TextEncoder();
    this.args = opts.args.map((a) => enc.encode(`${a}\0`));
    this.env = Object.entries(opts.env ?? {}).map(([k, v]) => enc.encode(`${k}=${v}\0`));
    this.fds.set(0, { kind: "stdio", fd: 0 });
    this.fds.set(1, { kind: "stdio", fd: 1 });
    this.fds.set(2, { kind: "stdio", fd: 2 });
    this.fds.set(3, { kind: "dir", node: opts.fs.root, preopen: "/" });
    this.imports = this.bind();
  }

  /** Run a command module's `_start`; returns its exit status. */
  start(instance: WebAssembly.Instance): number {
    this.memory = instance.exports.memory as WebAssembly.Memory;
    try {
      (instance.exports._start as () => void)();
      return 0;
    } catch (e) {
      if (e instanceof WasiExit) return e.code;
      throw e;
    }
  }

  stdout(): string {
    return decode(this.out);
  }

  stderr(): string {
    return decode(this.err);
  }

  /** For tests that drive the imports against their own memory. */
  useMemory(memory: WebAssembly.Memory): void {
    this.memory = memory;
  }

  private view(): DataView {
    if (!this.memory) throw new Error("WASI memory is not bound");
    return new DataView(this.memory.buffer);
  }

  // The memory is never shared: wasm32-wasip1 is single threaded.
  private bytes(ptr: number, len: number): Uint8Array<ArrayBuffer> {
    if (!this.memory) throw new Error("WASI memory is not bound");
    return new Uint8Array(this.memory.buffer as ArrayBuffer, ptr, len);
  }

  private str(ptr: number, len: number): string {
    return new TextDecoder().decode(this.bytes(ptr, len));
  }

  /** Resolve `path` under directory fd `fd`; ".." may not climb above it. */
  private resolve(fd: number, path: string): { dir: DirNode; parts: string[] } | number {
    const open = this.fds.get(fd);
    if (!open) return ERRNO.BADF;
    if (open.kind !== "dir") return ERRNO.NOTDIR;
    const parts: string[] = [];
    for (const part of path.split("/")) {
      if (part === "" || part === ".") continue;
      if (part === "..") {
        if (parts.length === 0) return ERRNO.ACCES;
        parts.pop();
      } else {
        parts.push(part);
      }
    }
    return { dir: open.node, parts };
  }

  private parentOf(fd: number, path: string): { parent: DirNode; name: string } | number {
    const r = this.resolve(fd, path);
    if (typeof r === "number") return r;
    const name = r.parts.pop();
    if (!name) return ERRNO.INVAL;
    const parent = this.opts.fs.lookup(r.dir, r.parts);
    if (!parent) return ERRNO.NOENT;
    if (parent.kind !== "dir") return ERRNO.NOTDIR;
    return { parent, name };
  }

  private writeFilestat(ptr: number, node: Node | null, filetype?: number): void {
    const v = this.view();
    for (let i = 0; i < 64; i += 8) v.setBigUint64(ptr + i, 0n, true);
    v.setBigUint64(ptr + 8, node?.ino ?? 0n, true);
    v.setUint8(ptr + 16, filetype ?? (node?.kind === "dir" ? FILETYPE_DIRECTORY : FILETYPE_REGULAR_FILE));
    v.setBigUint64(ptr + 24, 1n, true);
    v.setBigUint64(ptr + 32, BigInt(node?.kind === "file" ? node.size : 0), true);
    const t = node?.mtime ?? 0n;
    v.setBigUint64(ptr + 40, t, true);
    v.setBigUint64(ptr + 48, t, true);
    v.setBigUint64(ptr + 56, t, true);
  }

  private bind(): Record<string, (...a: never[]) => number | void> {
    const sizes = (list: Uint8Array[], countPtr: number, sizePtr: number) => {
      const v = this.view();
      v.setUint32(countPtr, list.length, true);
      v.setUint32(sizePtr, list.reduce((n, a) => n + a.byteLength, 0), true);
      return ERRNO.SUCCESS;
    };
    const fill = (list: Uint8Array[], ptrsPtr: number, bufPtr: number) => {
      const v = this.view();
      let at = bufPtr;
      list.forEach((a, i) => {
        v.setUint32(ptrsPtr + i * 4, at, true);
        this.bytes(at, a.byteLength).set(a);
        at += a.byteLength;
      });
      return ERRNO.SUCCESS;
    };
    const iovecs = (ptr: number, len: number) => {
      const v = this.view();
      return Array.from({ length: len }, (_, i) => ({
        buf: v.getUint32(ptr + i * 8, true),
        len: v.getUint32(ptr + i * 8 + 4, true),
      }));
    };

    const imports = {
      args_sizes_get: (countPtr: number, sizePtr: number) => sizes(this.args, countPtr, sizePtr),
      args_get: (ptrsPtr: number, bufPtr: number) => fill(this.args, ptrsPtr, bufPtr),
      environ_sizes_get: (countPtr: number, sizePtr: number) => sizes(this.env, countPtr, sizePtr),
      environ_get: (ptrsPtr: number, bufPtr: number) => fill(this.env, ptrsPtr, bufPtr),

      clock_time_get: (id: number, _precision: bigint, timePtr: number) => {
        const ns = id === 0 ? now() : BigInt(Math.round(performance.now() * 1_000_000));
        this.view().setBigUint64(timePtr, ns, true);
        return ERRNO.SUCCESS;
      },
      random_get: (ptr: number, len: number) => {
        for (let at = 0; at < len; at += 65536) {
          crypto.getRandomValues(this.bytes(ptr + at, Math.min(65536, len - at)));
        }
        return ERRNO.SUCCESS;
      },
      sched_yield: () => ERRNO.SUCCESS,
      proc_exit: (code: number): void => {
        throw new WasiExit(code);
      },
      // Single-threaded and never waiting on I/O: every subscription (clock
      // or fd) is reported ready at once.
      poll_oneoff: (inPtr: number, outPtr: number, n: number, neventsPtr: number) => {
        const v = this.view();
        for (let i = 0; i < n; i += 1) {
          const sub = inPtr + i * 48;
          const ev = outPtr + i * 32;
          v.setBigUint64(ev, v.getBigUint64(sub, true), true);
          v.setUint16(ev + 8, ERRNO.SUCCESS, true);
          v.setUint8(ev + 10, v.getUint8(sub + 8));
          v.setBigUint64(ev + 16, 0n, true);
          v.setUint16(ev + 24, 0, true);
        }
        v.setUint32(neventsPtr, n, true);
        return ERRNO.SUCCESS;
      },

      fd_prestat_get: (fd: number, ptr: number) => {
        const open = this.fds.get(fd);
        if (open?.kind !== "dir" || open.preopen === undefined) return ERRNO.BADF;
        const v = this.view();
        v.setUint8(ptr, 0);
        v.setUint32(ptr + 4, new TextEncoder().encode(open.preopen).byteLength, true);
        return ERRNO.SUCCESS;
      },
      fd_prestat_dir_name: (fd: number, ptr: number, len: number) => {
        const open = this.fds.get(fd);
        if (open?.kind !== "dir" || open.preopen === undefined) return ERRNO.BADF;
        this.bytes(ptr, len).set(new TextEncoder().encode(open.preopen).subarray(0, len));
        return ERRNO.SUCCESS;
      },
      fd_fdstat_get: (fd: number, ptr: number) => {
        const open = this.fds.get(fd);
        if (!open) return ERRNO.BADF;
        const v = this.view();
        const type =
          open.kind === "stdio"
            ? FILETYPE_CHARACTER_DEVICE
            : open.kind === "dir"
              ? FILETYPE_DIRECTORY
              : FILETYPE_REGULAR_FILE;
        v.setUint8(ptr, type);
        v.setUint16(ptr + 2, open.kind === "file" && open.append ? FDFLAGS_APPEND : 0, true);
        v.setBigUint64(ptr + 8, ALL_RIGHTS, true);
        v.setBigUint64(ptr + 16, ALL_RIGHTS, true);
        return ERRNO.SUCCESS;
      },
      fd_filestat_get: (fd: number, ptr: number) => {
        const open = this.fds.get(fd);
        if (!open) return ERRNO.BADF;
        if (open.kind === "stdio") this.writeFilestat(ptr, null, FILETYPE_CHARACTER_DEVICE);
        else this.writeFilestat(ptr, open.node);
        return ERRNO.SUCCESS;
      },
      fd_close: (fd: number) => {
        if (!this.fds.has(fd)) return ERRNO.BADF;
        this.fds.delete(fd);
        return ERRNO.SUCCESS;
      },
      fd_read: (fd: number, iovs: number, iovsLen: number, nreadPtr: number) => {
        const open = this.fds.get(fd);
        if (!open) return ERRNO.BADF;
        let total = 0;
        if (open.kind === "file") {
          for (const { buf, len } of iovecs(iovs, iovsLen)) {
            const n = Math.max(0, Math.min(len, open.node.size - open.offset));
            this.bytes(buf, n).set(open.node.data.subarray(open.offset, open.offset + n));
            open.offset += n;
            total += n;
            if (n < len) break;
          }
        } else if (open.kind === "dir") {
          return ERRNO.ISDIR;
        }
        this.view().setUint32(nreadPtr, total, true);
        return ERRNO.SUCCESS;
      },
      fd_write: (fd: number, iovs: number, iovsLen: number, nwrittenPtr: number) => {
        const open = this.fds.get(fd);
        if (!open) return ERRNO.BADF;
        if (open.kind === "dir") return ERRNO.ISDIR;
        let total = 0;
        for (const { buf, len } of iovecs(iovs, iovsLen)) {
          const chunk = this.bytes(buf, len);
          if (open.kind === "stdio") {
            if (open.fd === 0) return ERRNO.BADF;
            (open.fd === 1 ? this.out : this.err).push(chunk.slice());
          } else {
            const node = open.node;
            if (open.append) open.offset = node.size;
            const end = open.offset + len;
            if (end > node.data.byteLength) {
              const grown = new Uint8Array(Math.max(end, node.data.byteLength * 2));
              grown.set(node.data.subarray(0, node.size));
              node.data = grown;
            }
            node.data.set(chunk, open.offset);
            open.offset = end;
            node.size = Math.max(node.size, end);
            node.mtime = now();
          }
          total += len;
        }
        this.view().setUint32(nwrittenPtr, total, true);
        return ERRNO.SUCCESS;
      },
      fd_readdir: (fd: number, buf: number, bufLen: number, cookie: bigint, bufusedPtr: number) => {
        const open = this.fds.get(fd);
        if (!open) return ERRNO.BADF;
        if (open.kind !== "dir") return ERRNO.NOTDIR;
        const enc = new TextEncoder();
        // ".." names the directory itself: nothing here walks upward through it.
        const entries: [string, Node][] = [[".", open.node], ["..", open.node], ...open.node.entries];
        const chunks: Uint8Array[] = [];
        for (let i = Number(cookie); i < entries.length; i += 1) {
          const [name, node] = entries[i];
          const nameBytes = enc.encode(name);
          const head = new Uint8Array(24 + nameBytes.byteLength);
          const v = new DataView(head.buffer);
          v.setBigUint64(0, BigInt(i + 1), true);
          v.setBigUint64(8, node.ino, true);
          v.setUint32(16, nameBytes.byteLength, true);
          v.setUint8(20, node.kind === "dir" ? FILETYPE_DIRECTORY : FILETYPE_REGULAR_FILE);
          head.set(nameBytes, 24);
          chunks.push(head);
        }
        let used = 0;
        for (const chunk of chunks) {
          if (used >= bufLen) break;
          const n = Math.min(chunk.byteLength, bufLen - used);
          this.bytes(buf + used, n).set(chunk.subarray(0, n));
          used += n;
        }
        this.view().setUint32(bufusedPtr, used, true);
        return ERRNO.SUCCESS;
      },

      path_open: (
        fd: number,
        _dirflags: number,
        pathPtr: number,
        pathLen: number,
        oflags: number,
        _rightsBase: bigint,
        _rightsInheriting: bigint,
        fdflags: number,
        openedPtr: number,
      ) => {
        const r = this.resolve(fd, this.str(pathPtr, pathLen));
        if (typeof r === "number") return r;
        let node = this.opts.fs.lookup(r.dir, r.parts);
        if (node && oflags & OFLAGS_CREAT && oflags & OFLAGS_EXCL) return ERRNO.EXIST;
        if (!node) {
          if (!(oflags & OFLAGS_CREAT)) return ERRNO.NOENT;
          const p = this.parentOf(fd, this.str(pathPtr, pathLen));
          if (typeof p === "number") return p;
          node = this.opts.fs.newFile();
          p.parent.entries.set(p.name, node);
        }
        if (oflags & OFLAGS_DIRECTORY && node.kind !== "dir") return ERRNO.NOTDIR;
        let open: Open;
        if (node.kind === "dir") {
          if (oflags & OFLAGS_TRUNC) return ERRNO.ISDIR;
          open = { kind: "dir", node };
        } else {
          if (oflags & OFLAGS_TRUNC) {
            node.size = 0;
            node.data = new Uint8Array(0);
          }
          open = { kind: "file", node, offset: 0, append: (fdflags & FDFLAGS_APPEND) !== 0 };
        }
        const opened = this.nextFd++;
        this.fds.set(opened, open);
        this.view().setUint32(openedPtr, opened, true);
        return ERRNO.SUCCESS;
      },
      path_filestat_get: (fd: number, _flags: number, pathPtr: number, pathLen: number, ptr: number) => {
        const r = this.resolve(fd, this.str(pathPtr, pathLen));
        if (typeof r === "number") return r;
        const node = this.opts.fs.lookup(r.dir, r.parts);
        if (!node) return ERRNO.NOENT;
        this.writeFilestat(ptr, node);
        return ERRNO.SUCCESS;
      },
      path_create_directory: (fd: number, pathPtr: number, pathLen: number) => {
        const p = this.parentOf(fd, this.str(pathPtr, pathLen));
        if (typeof p === "number") return p;
        if (p.parent.entries.has(p.name)) return ERRNO.EXIST;
        p.parent.entries.set(p.name, this.opts.fs.newDir());
        return ERRNO.SUCCESS;
      },
      path_unlink_file: (fd: number, pathPtr: number, pathLen: number) => {
        const p = this.parentOf(fd, this.str(pathPtr, pathLen));
        if (typeof p === "number") return p;
        const node = p.parent.entries.get(p.name);
        if (!node) return ERRNO.NOENT;
        if (node.kind === "dir") return ERRNO.ISDIR;
        p.parent.entries.delete(p.name);
        return ERRNO.SUCCESS;
      },
      path_remove_directory: (fd: number, pathPtr: number, pathLen: number) => {
        const p = this.parentOf(fd, this.str(pathPtr, pathLen));
        if (typeof p === "number") return p;
        const node = p.parent.entries.get(p.name);
        if (!node) return ERRNO.NOENT;
        if (node.kind !== "dir") return ERRNO.NOTDIR;
        if (node.entries.size > 0) return ERRNO.NOTEMPTY;
        p.parent.entries.delete(p.name);
        return ERRNO.SUCCESS;
      },
      // The tree holds no symlinks.
      path_readlink: (fd: number, pathPtr: number, pathLen: number) => {
        const r = this.resolve(fd, this.str(pathPtr, pathLen));
        if (typeof r === "number") return r;
        return this.opts.fs.lookup(r.dir, r.parts) ? ERRNO.INVAL : ERRNO.NOENT;
      },
    };
    return imports as unknown as Record<string, (...a: never[]) => number | void>;
  }
}

function decode(chunks: Uint8Array[]): string {
  const total = chunks.reduce((n, c) => n + c.byteLength, 0);
  const all = new Uint8Array(total);
  let at = 0;
  for (const c of chunks) {
    all.set(c, at);
    at += c.byteLength;
  }
  return new TextDecoder().decode(all);
}

export const WASI_ERRNO = ERRNO;
