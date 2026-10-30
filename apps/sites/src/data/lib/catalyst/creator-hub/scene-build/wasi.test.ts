import { describe, expect, it } from "vitest";

import { MemFs, Wasi, WASI_ERRNO as E } from "./wasi";

// The shim's syscalls, driven directly against a scratch memory the way
// wasi-libc calls them: arguments are pointers, results land in memory.

function harness() {
  const fs = new MemFs();
  const wasi = new Wasi({ args: ["cmd", "/scene"], env: { A: "1" }, fs });
  const memory = new WebAssembly.Memory({ initial: 1 });
  wasi.useMemory(memory);
  const view = () => new DataView(memory.buffer);
  const bytes = () => new Uint8Array(memory.buffer);
  let top = 1024;
  const put = (data: string | Uint8Array): [number, number] => {
    const b = typeof data === "string" ? new TextEncoder().encode(data) : data;
    const at = top;
    bytes().set(b, at);
    top += b.byteLength + 8 - (b.byteLength % 8);
    return [at, b.byteLength];
  };
  const call = (name: string, ...args: (number | bigint)[]) =>
    (wasi.imports[name] as unknown as (...a: (number | bigint)[]) => number)(...args);
  const OUT = 64;
  const open = (path: string, oflags: number, fdflags = 0) => {
    const [p, n] = put(path);
    const errno = call("path_open", 3, 0, p, n, oflags, 0n, 0n, fdflags, OUT);
    return { errno, fd: view().getUint32(OUT, true) };
  };
  const write = (fd: number, text: string) => {
    const [p, n] = put(text);
    const iov = top;
    view().setUint32(iov, p, true);
    view().setUint32(iov + 4, n, true);
    top += 8;
    return call("fd_write", fd, iov, 1, OUT);
  };
  const read = (fd: number, len: number) => {
    const [buf] = put(new Uint8Array(len));
    const iov = top;
    view().setUint32(iov, buf, true);
    view().setUint32(iov + 4, len, true);
    top += 8;
    expect(call("fd_read", fd, iov, 1, OUT)).toBe(E.SUCCESS);
    return new TextDecoder().decode(bytes().slice(buf, buf + view().getUint32(OUT, true)));
  };
  return { fs, wasi, view, bytes, put, call, open, write, read, OUT };
}

describe("the scene-build WASI shim", () => {
  it("preopens / as fd 3 and hands out args and environment", () => {
    const h = harness();
    expect(h.call("fd_prestat_get", 3, h.OUT)).toBe(E.SUCCESS);
    expect(h.view().getUint32(h.OUT + 4, true)).toBe(1);
    expect(h.call("fd_prestat_get", 4, h.OUT)).toBe(E.BADF);
    expect(h.call("args_sizes_get", h.OUT, h.OUT + 4)).toBe(E.SUCCESS);
    expect(h.view().getUint32(h.OUT, true)).toBe(2);
    expect(h.view().getUint32(h.OUT + 4, true)).toBe("cmd\0/scene\0".length);
    expect(h.call("environ_sizes_get", h.OUT, h.OUT + 4)).toBe(E.SUCCESS);
    expect(h.view().getUint32(h.OUT, true)).toBe(1);
  });

  it("creates, writes, stats, reads, lists and unlinks files under the preopen", () => {
    const h = harness();
    h.fs.writeFile("scene/src/index.ts", new TextEncoder().encode("export {}\n"));
    const [dir, dirLen] = h.put("scene/bin");
    expect(h.call("path_create_directory", 3, dir, dirLen)).toBe(E.SUCCESS);
    expect(h.call("path_create_directory", 3, dir, dirLen)).toBe(E.EXIST);

    const created = h.open("scene/bin/index.js", 1 | 8);
    expect(created.errno).toBe(E.SUCCESS);
    expect(h.write(created.fd, "hello ")).toBe(E.SUCCESS);
    expect(h.write(created.fd, "world")).toBe(E.SUCCESS);
    expect(h.call("fd_close", created.fd)).toBe(E.SUCCESS);
    expect(new TextDecoder().decode(h.fs.readFile("scene/bin/index.js")!)).toBe("hello world");

    const reopened = h.open("./scene/src/../bin/index.js", 0);
    expect(reopened.errno).toBe(E.SUCCESS);
    expect(h.call("fd_filestat_get", reopened.fd, h.OUT)).toBe(E.SUCCESS);
    expect(h.view().getUint8(h.OUT + 16)).toBe(4);
    expect(h.view().getBigUint64(h.OUT + 32, true)).toBe(11n);
    expect(h.read(reopened.fd, 5)).toBe("hello");
    expect(h.read(reopened.fd, 64)).toBe(" world");
    expect(h.read(reopened.fd, 64)).toBe("");

    const listing = h.open("scene", 2);
    expect(listing.errno).toBe(E.SUCCESS);
    const [buf] = h.put(new Uint8Array(512));
    expect(h.call("fd_readdir", listing.fd, buf, 512, 0n, h.OUT)).toBe(E.SUCCESS);
    const used = h.view().getUint32(h.OUT, true);
    const names: string[] = [];
    for (let at = buf; at < buf + used; ) {
      const len = h.view().getUint32(at + 16, true);
      names.push(new TextDecoder().decode(h.bytes().slice(at + 24, at + 24 + len)));
      at += 24 + len;
    }
    expect(names).toEqual([".", "..", "src", "bin"]);

    const [file, fileLen] = h.put("scene/bin/index.js");
    expect(h.call("path_unlink_file", 3, file, fileLen)).toBe(E.SUCCESS);
    expect(h.call("path_filestat_get", 3, 0, file, fileLen, h.OUT)).toBe(E.NOENT);
    expect(h.call("path_remove_directory", 3, dir, dirLen)).toBe(E.SUCCESS);
  });

  it("refuses paths that climb out of the preopen and opens that need a missing parent", () => {
    const h = harness();
    expect(h.open("../etc/passwd", 0).errno).toBe(E.ACCES);
    expect(h.open("missing/dir/file.js", 1).errno).toBe(E.NOENT);
    expect(h.open("nothing.js", 0).errno).toBe(E.NOENT);
    h.fs.writeFile("a.js", new Uint8Array([1]));
    expect(h.open("a.js", 2).errno).toBe(E.NOTDIR);
    expect(h.open("a.js", 1 | 4).errno).toBe(E.EXIST);
  });

  it("captures stdout and stderr and turns proc_exit into an exit status", () => {
    const h = harness();
    h.write(1, '{"ok":true}');
    h.write(2, "boom");
    expect(h.wasi.stdout()).toBe('{"ok":true}');
    expect(h.wasi.stderr()).toBe("boom");
    const instance = {
      exports: {
        memory: new WebAssembly.Memory({ initial: 1 }),
        _start: () => h.call("proc_exit", 3),
      },
    } as unknown as WebAssembly.Instance;
    expect(h.wasi.start(instance)).toBe(3);
  });
});
