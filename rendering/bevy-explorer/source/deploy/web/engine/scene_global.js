// The scene worker global: what scene code may name, and what is removed before it runs.
// Kept apart from sandbox_worker.js, whose bundle must export nothing.

export const allowListES2020 = [
  "Array",
  "ArrayBuffer",
  "BigInt",
  "BigInt64Array",
  "BigUint64Array",
  "Boolean",
  "DataView",
  "Date",
  "decodeURI",
  "decodeURIComponent",
  "encodeURI",
  "encodeURIComponent",
  "Error",
  "escape",
  "eval",
  "EvalError",
  "fetch",
  "Float32Array",
  "Float64Array",
  "Function",
  "Infinity",
  "Int16Array",
  "Int32Array",
  "Int8Array",
  "isFinite",
  "isNaN",
  "JSON",
  "Map",
  "Math",
  "NaN",
  "Number",
  "Object",
  "parseFloat",
  "parseInt",
  "Promise",
  "Proxy",
  "RangeError",
  "ReferenceError",
  "Reflect",
  "RegExp",
  "Set",
  "SharedArrayBuffer",
  "String",
  "Symbol",
  "SyntaxError",
  "TypeError",
  "Uint16Array",
  "Uint32Array",
  "Uint8Array",
  "Uint8ClampedArray",
  "undefined",
  "unescape",
  "URIError",
  "WeakMap",
  "WebSocket",
  "WeakSet",
];

const TIMERS = ["setTimeout", "clearTimeout", "setInterval", "clearInterval"];

// The scene's globalThis over `context`, and the preamble that declares each name it offers.
export function createSceneScope(context) {
  // A WebIDL timer called as `globalThis.setTimeout(...)` would get the proxy as `this` and throw.
  for (const name of TIMERS) {
    Object.defineProperty(context, name, { configurable: false, value: self[name].bind(self) });
  }
  const proxy = new Proxy(context, {
    has() {
      return true;
    },
    get(_target, propKey, _receiver) {
      if (propKey === "eval") return eval;
      if (propKey === "globalThis") return proxy;
      if (propKey === "global") return proxy;
      if (propKey === "undefined") return undefined;
      if (context[propKey] !== undefined) return context[propKey];
      if (allowListES2020.includes(propKey)) {
        return globalThis[propKey];
      }
      return undefined;
    },
  });

  const contextKeys = Object.getOwnPropertyNames(context);
  const allGlobals = [...new Set([...allowListES2020, ...contextKeys])];
  const preamble = allGlobals
    .map((key) => `const ${key} = globalThis.${key};`)
    .join("\n");
  return { proxy, preamble };
}

// Remove a property from the object and every prototype that holds it. Interface objects
// (BroadcastChannel, Worker) are own properties of the global; attributes like navigator.storage
// live on a prototype, and deleting them off the instance silently succeeds without removing
// anything.
function deleteFromPrototypeChain(obj, name) {
  for (let o = obj; o != null; o = Object.getPrototypeOf(o)) delete o[name];
}

// Withhold from the worker global what scene code must not reach. Runs before any scene code.
export function scrubSceneGlobal() {
  // The allowlist below cannot withhold a capability: jsProxy is only consulted for
  // `globalThis.X` property lookups, so a bare identifier in scene code resolves straight
  // through to the real worker global. Withholding therefore has to be a deletion from that
  // global. `Worker` goes with it — a nested worker is a fresh realm whose global has
  // BroadcastChannel back, which would undo the deletion in one line. (SharedWorker isn't
  // exposed to dedicated workers in Chromium, but delete it too for other engines.)
  //
  // Runs before preloadModules and before any scene code, so nothing untrusted has observed
  // the pre-scrub global. Deleting for super-user scenes as well keeps one code path: the
  // trusted scene gets the captured constructor back through jsContext, below.
  delete self.BroadcastChannel;
  delete self.Worker;
  delete self.SharedWorker;

  // OPFS is the origin's storage: config.json and the ipfs cache hang off the one root that
  // navigator.storage.getDirectory() hands out. Scenes get no persistent storage of any kind.
  //
  // `delete navigator.storage` would return true and do nothing: WorkerNavigator exposes it on its
  // prototype, so the own-property delete succeeds against a property that was never there. Walk to
  // the prototype that actually holds it. (Same reason `delete self.navigator` is a no-op.)
  //
  // storageBuckets is the second door to the same API — navigator.storageBuckets.open(name) hands
  // back a bucket with its own getDirectory(). A named bucket can't reach the default bucket's
  // contents, so it isn't a route to engine state, but it is unmetered scene-controlled storage and
  // two scenes agreeing on a bucket name would have a shared filesystem.
  deleteFromPrototypeChain(self.navigator, "storage");
  deleteFromPrototypeChain(self.navigator, "storageBuckets");
  // the page's service worker registration (and its cache routes) is no scene's business
  deleteFromPrototypeChain(self.navigator, "serviceWorker");
  // the scene server's one-per-scene lock (headless.js) is held by name, which a scene could take
  deleteFromPrototypeChain(self.navigator, "locks");
  // devices the origin was granted are no scene's business either
  for (const name of ["usb", "hid", "serial"]) deleteFromPrototypeChain(self.navigator, name);

  // IndexedDB is same-origin too, and holds more than its own data: platform/src/web_save.js keeps
  // the FileSystemDirectoryHandle for the user's picked scene folder there (db `dcl-editor`, store
  // `handles`), with readwrite permission already granted. A handle read back out of IndexedDB is
  // as live as the one that was stored, so a scene reaching it would get the user's real
  // filesystem, not origin-private storage. Nothing in this worker uses IndexedDB — web_save.js
  // runs on the main thread and gpu_cache.js on the render worker.
  deleteFromPrototypeChain(self, "indexedDB");

  // CacheStorage is the last same-origin store the sandbox could see — it holds the ipfs fetch
  // cache (`ipfs-path-cache-v2`), so a scene could read every asset the client has pulled and, more
  // to the point, write to keys the loader later serves. Its users are elsewhere:
  // image_processing/src/processor/wasm_fs.rs runs on the asset processor worker, which engine.js spawns
  // as its own worker, and service_worker.js is a different context entirely.
  deleteFromPrototypeChain(self, "caches");

  // Requests that bypass createSceneFetch, its send limit and its credentials policy.
  deleteFromPrototypeChain(self, "XMLHttpRequest");
  deleteFromPrototypeChain(self, "EventSource");
  // A module worker's importScripts only throws, but it is still a way to load a url.
  deleteFromPrototypeChain(self, "importScripts");
  // Older persistent stores, where a browser still exposes them to workers.
  for (const name of [
    "webkitRequestFileSystem",
    "webkitRequestFileSystemSync",
    "webkitResolveLocalFileSystemURL",
    "webkitResolveLocalFileSystemSyncURL",
    "cookieStore",
  ]) {
    deleteFromPrototypeChain(self, name);
  }
}
