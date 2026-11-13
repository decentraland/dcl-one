// The preview's type checker: one node process that keeps tsc loaded and the
// last program in memory, so a re-check parses and checks only what an edit
// touched. It runs tsc's own command line (`executeCommandLine`) with the
// arguments a spawned tsc would get, so the report is tsc's, byte for byte.
//
// Last argument: the scene's typescript/lib/tsc.js. One JSON request per stdin line,
// `{"id": n, "args": [...]}`; one JSON reply per stdout line, `{"id": n,
// "code": exit status, "out": report}`. The first line out is `{"ready":
// true}` or `{"unsupported": why}`; the process ends when stdin closes.
"use strict";
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { createRequire } = require("node:module");

const say = (message) => fs.writeSync(1, JSON.stringify(message) + "\n");

// tsc 5.x and 6.x are one bundled script ending in this call; the script is
// compiled without it, with `serve` (below) in its scope instead. 7.x's
// tsc.js launches a native binary and has no such line.
const ENTRY = "executeCommandLine(sys, noop, sys.args);";
const NEEDS = [
  "executeCommandLine",
  "readBuilderProgram",
  "createIncrementalCompilerHost",
  "sys",
  "noop",
];

function load(shim) {
  const dir = path.dirname(shim);
  const bundled = path.join(dir, "_tsc.js");
  const file = fs.existsSync(bundled) ? bundled : shim;
  const source = fs.readFileSync(file, "utf8");
  const at = source.lastIndexOf(ENTRY);
  if (at < 0) return { unsupported: "tsc.js is not the bundled JavaScript compiler" };
  const probe = NEEDS.map((name) => `typeof ${name} !== "undefined"`).join(" && ");
  const hooks = `
return (${probe}) && {
  executeCommandLine, sys, noop,
  readBuilderProgram, setReadBuilderProgram(f) { readBuilderProgram = f; },
  createIncrementalCompilerHost, setCreateIncrementalCompilerHost(f) { createIncrementalCompilerHost = f; },
};`;
  const body = source.slice(0, at) + hooks;
  const run = vm.compileFunction(body, ["exports", "require", "module", "__filename", "__dirname"], {
    filename: file,
  });
  const module_ = { exports: {} };
  const ts = run(module_.exports, createRequire(file), module_, file, dir);
  return ts ? { ts } : { unsupported: "this tsc.js lacks the functions the checker drives" };
}

function serve(ts) {
  let out = "";
  let code = 0;
  /** The last run's builder program and the options and arguments it ran with. */
  let last = null;
  let lastArgs = "";
  let building = null;
  /** fileName -> { file, key, size, mtime } for every source file parsed. */
  const parsed = new Map();
  /** The files the run in hand asked for; the rest are dropped after it. */
  const used = new Set();

  const system = {
    ...ts.sys,
    write: (text) => void (out += text),
    writeOutputIsTTY: () => false,
    exit: (status) => void (code = status || 0),
  };

  const forget = () => {
    last = null;
    parsed.clear();
  };

  // A dependency is trusted by size and mtime; the scene's own files are
  // re-read, since two saves inside one tick of a coarse clock can share
  // both.
  const fresh = (fileName, entry, host) => {
    if (fileName.includes("/node_modules/")) {
      const stat = fs.statSync(fileName, { bigint: true, throwIfNoEntry: false });
      return !!stat && stat.size === entry.size && stat.mtimeNs === entry.mtime;
    }
    return host.readFile(fileName) === entry.file.text;
  };

  const readBuilderProgram = ts.readBuilderProgram;
  ts.setReadBuilderProgram((options, host) => {
    const wanted = JSON.stringify(options);
    if (last && last.options === wanted) return last.program;
    forget();
    building = wanted;
    return readBuilderProgram(options, host);
  });

  const createIncrementalCompilerHost = ts.createIncrementalCompilerHost;
  ts.setCreateIncrementalCompilerHost((options, sys) => {
    const host = createIncrementalCompilerHost(options, sys);
    building = JSON.stringify(options);
    if (last && last.options !== building) forget();
    const getSourceFile = host.getSourceFile;
    host.getSourceFile = (fileName, languageVersionOrOptions, onError, shouldCreate) => {
      const key = JSON.stringify(languageVersionOrOptions);
      used.add(fileName);
      const entry = parsed.get(fileName);
      if (entry && !shouldCreate && entry.key === key && fresh(fileName, entry, host)) {
        return entry.file;
      }
      const file = getSourceFile(fileName, languageVersionOrOptions, onError, shouldCreate);
      const stat = file && fs.statSync(fileName, { bigint: true, throwIfNoEntry: false });
      if (stat) parsed.set(fileName, { file, key, size: stat.size, mtime: stat.mtimeNs });
      else parsed.delete(fileName);
      return file;
    };
    return host;
  });

  return (args) => {
    out = "";
    code = 0;
    const these = JSON.stringify(args);
    if (these !== lastArgs) forget();
    lastArgs = these;
    building = null;
    used.clear();
    let program = null;
    try {
      ts.executeCommandLine(system, (built) => void (program = built), args);
    } catch (e) {
      forget();
      throw e;
    }
    last = program && building && program.getProgram ? { program, options: building } : null;
    if (!last) parsed.clear();
    for (const fileName of parsed.keys()) if (!used.has(fileName)) parsed.delete(fileName);
    return { code, out };
  };
}

function main() {
  let check;
  try {
    const loaded = load(process.argv.at(-1));
    if (!loaded.ts) return say({ unsupported: loaded.unsupported });
    check = serve(loaded.ts);
  } catch (e) {
    return say({ unsupported: String((e && e.message) || e) });
  }
  say({ ready: true });
  let pending = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (chunk) => {
    pending += chunk;
    for (let nl = pending.indexOf("\n"); nl >= 0; nl = pending.indexOf("\n")) {
      const line = pending.slice(0, nl);
      pending = pending.slice(nl + 1);
      if (!line.trim()) continue;
      let id = null;
      try {
        const request = JSON.parse(line);
        id = request.id;
        say({ id, ...check(request.args) });
      } catch (e) {
        say({ id, error: String((e && e.stack) || e) });
      }
    }
  });
  process.stdin.on("end", () => process.exit(0));
}

main();
