// this code is executed as the runtime is created, all scopes get these definitions

import { internals } from "ext:core/mod.js";

// required for async ops (engine.sendMessage is declared as async)
// Deno.core.initializeAsyncOps();

// Private handle on the runtime internals, captured before they are removed from the
// global object (see `sealRealm` below). `require` is the only thing that keeps a
// reference, and it hands it out only to `~system/*` modules -- never to scene code.
const denoRef = Deno;

// Scene code can replace any built-in it can reach, so `require` uses only these copies,
// taken before the scene runs, on the paths that touch `denoRef`.
import { primordials } from "ext:core/mod.js";
const { ReflectApply, StringPrototypeCharCodeAt, StringPrototypeSlice } = primordials;
const { op_eval_context: opEvalContext, op_require: opRequire } = denoRef.core.ops;

// The scene bundle is untrusted code that runs in THIS realm, so every global reachable
// here is reachable from it -- including the raw ops table, which is the whole API surface
// with none of the checks the `~system/*` wrappers apply. Drop the routes to it just
// before the scene is evaluated (not at load time: the deno ext modules read these during
// their own module evaluation, which happens first).
//
// `~system/*` modules still need the ops table, so `require` passes `denoRef` in as the
// `Deno` parameter of their wrapper function. They are unchanged and see `Deno` as before;
// the scene gets `undefined` for that parameter and no global to fall back to.
//
// AUTHORITATIVE SERVER ONLY. That is where scenes are multi-tenant and the host has
// credentials and a network position worth reaching, so the reward for a deployed scene
// escaping into the ops table is real. The desktop client runs on the user's own machine
// and keeps today's behaviour, so a scene that reaches `Deno` there is not a regression
// against a working preview.
function isAuthoritativeServer() {
    return !!(denoRef.core.ops.op_is_server && denoRef.core.ops.op_is_server());
}

let realmSealed = false;
function sealRealm() {
    if (realmSealed) {
        return;
    }
    realmSealed = true;
    delete globalThis.Deno;        // Deno.core.ops
    delete globalThis.__bootstrap; // { core, internals, primordials } -- same ops table
    delete globalThis.__infra;
}

// load a cjs/node-style module
// TODO: consider using deno.land/std/node's `createRequire` directly.
// Deno's node polyfill doesn't work without the full deno runtime, and i
// note that decentraland examples use ESM syntax which deno_core does support,
// so i haven't gone very deep into making full support work.
// this is a very simplified version of the deno_std/node `createRequire` implementation.
// Drops a leading `#!` line, which is only valid at the very start of a script.
function stripHashbang(source) {
    if (StringPrototypeCharCodeAt(source, 0) !== 0x23 || StringPrototypeCharCodeAt(source, 1) !== 0x21) {
        return source;
    }
    for (let i = 2; i < source.length; i++) {
        const c = StringPrototypeCharCodeAt(source, i);
        if (c === 0x0a) {
            return StringPrototypeSlice(source, i + 1);
        }
        if (c === 0x0d || c === 0x2028 || c === 0x2029) {
            return source;
        }
    }
    return source;
}

function require(moduleName) {
    moduleName = `${moduleName}`;
    // dynamically load the module source
    var source = opRequire(moduleName);

    // create a wrapper for the imported script
    source = stripHashbang(source);
    const head = "(function (exports, require, module, __filename, __dirname, Deno) {";
    const foot = "\n})";
    source = `${head}${source}${foot}`;

    // the scene bundle is the untrusted one; everything else here is ours. Hiding the
    // internals from it is authoritative-server only -- on the client the scene keeps
    // seeing exactly what it sees today, global and wrapper parameter alike.
    const hideInternals = moduleName === "~scene.js" && isAuthoritativeServer();
    if (hideInternals) {
        sealRealm();
    }

    // [result, null] or [null, [thrown, isNativeError, isCompileError]]
    const evaluated = opEvalContext(source, "file://${moduleName}");
    if (evaluated[1]) {
        throw evaluated[1][0];
    }
    const wrapped = evaluated[0];

    // create minimal context for the execution
    var module = {
        exports: {}
    };
    // call the script
    // note: `require` function base path would need to be updated for proper support
    ReflectApply(wrapped, module.exports, [
        module.exports,                      // exports
        require,                             // require
        module,                              // module
        StringPrototypeSlice(moduleName, 1),    // __filename
        StringPrototypeSlice(moduleName, 0, 1), // __dirname
        hideInternals ? undefined : denoRef, // Deno
    ]);

    return module.exports;
}

// minimal console
function customLog(...values) {
    return values.map(value => logValue(value, new WeakSet())).join(' ')
}

function logValue(value, seen) {
    const valueType = typeof value
    if (valueType === 'number' || valueType === 'string' || valueType === 'boolean') {
        return JSON.stringify(value)
    } else if (valueType === 'function') {
        return '[Function]'
    } else if (value === null) {
        return 'null'
    } else if (Array.isArray(value)) {
        if (seen.has(value)) {
            return '[CircularArray]';
        } else {
            seen.add(value);
            return `Array(${value.length}) [${value.map(item => logValue(item, seen)).join(', ')}]`;
        }
    } else if (valueType === 'object') {
        if (seen.has(value)) {
            return '[CircularObject]'
        } else {
            seen.add(value);

            const objName = value.constructor?.name ?? 'Object'
            if (objName === 'Object') {
                return `Object {${Object.keys(value).map(key => `${key}: ${logValue(value[key], seen)}`).join(', ')}}`;
            } else {
                if (value instanceof Error) {
                    return `[${objName} ${value.message} ${value.stack}`;
                } else {
                    return `${objName} {${Object.keys(value).map(key => `${key}: ${logValue(value[key], seen)}`).join(', ')}}`;
                }
            }
        }
    } else if (valueType === 'symbol') {
        return `Symbol (${value.toString()})`;
    } else if (valueType === 'bigint') {
        return `BigInt (${value.toString()})`;
    } else if (valueType === 'undefined') {
        return 'undefined';
    } else {
        return `[Unsupported Type = ${valueType} toString() ${value?.toString ? value.toString() : 'none'} valueOf() ${value}}]`;
    }
}

const console = {
    trace: function (...args) {
        denoRef.core.ops.op_log("TRACE " + customLog(...args))
    },
    log: function (...args) {
        denoRef.core.ops.op_log("LOG " + customLog(...args))
    },
    error: function (...args) {
        denoRef.core.ops.op_error("ERROR " + customLog(...args))
    },
    warn: function (...args) {
        denoRef.core.ops.op_log("WARN " + customLog(...args))
    },
}

// timeout handler
globalThis.setImmediate = (fn) => Promise.resolve().then(fn)

globalThis.require = require;
globalThis.console = console;
globalThis.self = {};

// The deno_web scripts load lazily; everything the scene uses is loaded here, before
// `sealRealm` removes the internals they read.
const loadExtScript = denoRef.core.loadExtScript;

// Fetch's AbortSignal uses the DOMException global when a caller aborts or times out.
const { DOMException } = loadExtScript("ext:deno_web/01_dom_exception.js");
const { AbortController, AbortSignal } = loadExtScript("ext:deno_web/03_abort_signal.js");
globalThis.DOMException = DOMException;
globalThis.AbortController = AbortController;
globalThis.AbortSignal = AbortSignal;

// A key the scripts read and nobody set would walk to Object.prototype, where a scene getter
// would get `internals` as `this`.
Object.setPrototypeOf(internals, null);

// The fetch scripts read their tracing hooks from these; scenes are never traced.
internals.__telemetry ??= {
    TRACING_ENABLED: false,
    DID_NOT_ENTER: Symbol("DID_NOT_ENTER"),
    PROPAGATORS: [],
    ContextManager: { active() { return undefined; } },
    builtinTracer() { return undefined; },
    enterSpan() { return undefined; },
    exitSpan() {},
};
internals.__telemetryUtil ??= {};

const { Request } = loadExtScript("ext:deno_fetch/23_request.js");
globalThis.Request = Request;

const { fetch } = loadExtScript("ext:deno_fetch/26_fetch.js");
// On the authoritative server a 3xx is never followed: the scene gets the redirect response
// back and re-issues the request itself. Decided per call, as the scene context is not in
// place while this module is evaluated. The Request is built first so an `init` read through
// getters (a Request, a class instance) keeps its method, headers and body.
globalThis.fetch = (input, init) => {
    if (!isAuthoritativeServer()) {
        return fetch(input, init);
    }
    let request;
    try {
        request = new Request(input, init);
    } catch (e) {
        return Promise.reject(e);
    }
    return fetch(request, { redirect: "manual" });
};

const timers = loadExtScript("ext:deno_web/02_timers.js");
globalThis.setTimeout = timers.setTimeout;
globalThis.setInterval = timers.setInterval;
globalThis.clearTimeout = timers.clearTimeout;

import * as websocket from "ext:BevyExplorer/websocket.js";
globalThis.WebSocket = websocket.WebSocket;

const textEncoding = loadExtScript("ext:deno_web/08_text_encoding.js");
globalThis.TextEncoder = textEncoding.TextEncoder;
globalThis.TextDecoder = textEncoding.TextDecoder;

const performance = loadExtScript("ext:deno_web/15_performance.js");
performance.setTimeOrigin();
globalThis.performance = performance.performance;

// `Date`, `Temporal.Now`, `Intl.DateTimeFormat` and `File` read the wall clock in V8, not
// through an op; floor what they read to the ops' granularity (CLOCK_GRANULARITY in
// hardened_ops.rs).
const NativeDate = Date;
const nativeNow = Date.now;
const ReflectConstruct = Reflect.construct;
const coarseNow = () => {
    // CLOCK_GRANULARITY in hardened_ops.rs
    const now = nativeNow();
    return now - (now % 2);
};
const CoarseDate = function Date(...args) {
    if (new.target === undefined) {
        return new NativeDate(coarseNow()).toString();
    }
    return ReflectConstruct(NativeDate, args.length === 0 ? [coarseNow()] : args, new.target);
};
Object.defineProperties(CoarseDate, {
    length: { value: NativeDate.length },
    prototype: { value: NativeDate.prototype, writable: false },
    now: { value: coarseNow, writable: true, configurable: true },
    parse: { value: NativeDate.parse, writable: true, configurable: true },
    UTC: { value: NativeDate.UTC, writable: true, configurable: true },
});
Object.defineProperty(NativeDate.prototype, "constructor", { value: CoarseDate });
globalThis.Date = CoarseDate;

const dateTimeFormat = Intl.DateTimeFormat.prototype;
const nativeFormat = Object.getOwnPropertyDescriptor(dateTimeFormat, "format").get;
const nativeFormatToParts = dateTimeFormat.formatToParts;
const orNow = (date) => (date === undefined ? coarseNow() : date);
const boundFormats = new WeakMap();
Object.defineProperties(dateTimeFormat, {
    format: {
        get() {
            let bound = boundFormats.get(this);
            if (bound === undefined) {
                const format = ReflectApply(nativeFormat, this, []);
                bound = (date) => format(orNow(date));
                boundFormats.set(this, bound);
            }
            return bound;
        },
        configurable: true,
    },
    formatToParts: {
        value: function formatToParts(date) {
            return ReflectApply(nativeFormatToParts, this, [orNow(date)]);
        },
        writable: true,
        configurable: true,
    },
});

// File defaults lastModified from the primordial Date, taken before CoarseDate, and every
// File reads it through this getter. Explicit values are floored too.
{
    const filePrototype = loadExtScript("ext:deno_web/09_file.js").File.prototype;
    const descriptor = Object.getOwnPropertyDescriptor(filePrototype, "lastModified");
    const nativeLastModified = descriptor.get;
    descriptor.get = function lastModified() {
        // CLOCK_GRANULARITY in hardened_ops.rs
        const time = ReflectApply(nativeLastModified, this, []);
        return time - (((time % 2) + 2) % 2);
    };
    Object.defineProperty(filePrototype, "lastModified", descriptor);
}

if (typeof Temporal === "object") {
    const { Instant, Now } = Temporal;
    const fromEpochMilliseconds = Instant.fromEpochMilliseconds;
    const timeZoneId = Now.timeZoneId;
    const toZonedDateTimeISO = Instant.prototype.toZonedDateTimeISO;
    const ZonedDateTime = Temporal.ZonedDateTime.prototype;
    const toPlainDateTime = ZonedDateTime.toPlainDateTime;
    const toPlainDate = ZonedDateTime.toPlainDate;
    const toPlainTime = ZonedDateTime.toPlainTime;
    const instant = () => fromEpochMilliseconds(coarseNow());
    const zoned = (timeZone) =>
        ReflectApply(toZonedDateTimeISO, instant(), [timeZone === undefined ? timeZoneId() : timeZone]);
    const methods = {
        instant,
        zonedDateTimeISO: zoned,
        plainDateTimeISO: (timeZone) => ReflectApply(toPlainDateTime, zoned(timeZone), []),
        plainDateISO: (timeZone) => ReflectApply(toPlainDate, zoned(timeZone), []),
        plainTimeISO: (timeZone) => ReflectApply(toPlainTime, zoned(timeZone), []),
    };
    for (const [name, method] of Object.entries(methods)) {
        Object.defineProperty(Now, name, { value: method, writable: true, configurable: true });
    }
}

// deno_core's wasm streaming handler needs a JS callback the runtime never registers, and
// scenes have no use for streaming compilation.
delete WebAssembly.compileStreaming;
delete WebAssembly.instantiateStreaming;

// Native super-user bridge transport. The web super-user scene gets a real BroadcastChannel from the
// browser; native deno has none, so expose one backed by host ops (op_bridge_to_page + the page->scene
// stream). Defined unconditionally (the ops aren't attached to Deno.core.ops yet at this esm-entry
// load time); each method guards on the op existing, so it's a live channel only in super-user scenes
// (where the ops are registered) and inert elsewhere. Lets the bridge-scene run unchanged on native.
globalThis.BroadcastChannel = class BroadcastChannel {
    constructor(name) {
        this.name = name;
        this.onmessage = null;
        this._closed = false;
        const self = this;
        if (denoRef.core.ops.op_get_bridge_stream) {
            denoRef.core.ops.op_get_bridge_stream().then(function (rid) {
                (async function () {
                    while (!self._closed) {
                        const s = await denoRef.core.ops.op_read_bridge_stream(rid);
                        if (!s) break; // "" => stream closed
                        if (self.onmessage) {
                            try { self.onmessage({ data: JSON.parse(s) }); } catch (e) {}
                        }
                    }
                })();
            });
        }
    }
    postMessage(msg) {
        if (denoRef.core.ops.op_bridge_to_page) {
            try { denoRef.core.ops.op_bridge_to_page(JSON.stringify(msg)); } catch (e) {}
        }
    }
    addEventListener(type, fn) { if (type === 'message') this.onmessage = fn; }
    removeEventListener() {}
    close() { this._closed = true; }
};

denoRef.core.setUnhandledPromiseRejectionHandler((promise, reason) => {
    console.error('Unhandled promise rejection: ', reason)
    return true;
})
