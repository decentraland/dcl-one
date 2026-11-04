// Ordinary trusted HTTP only. IPFS downloads and scene Fetch have their own budgets.
const PRESSURE_THRESHOLD = 15;
const NO_PROGRESS_MS = 5000;

export function createCoreHttpHost(now = () => performance.now()) {
  const active = new Map();
  let highWater = 0;
  let lastProgress = 0;

  function scope(parent) {
    const tokens = new Map();
    const children = new Set();
    let disposed = false;
    let nextJsId = -1; // WASM IDs are unsigned; HUD and page bundles share this sequence.

    function begin(id) {
      if (disposed || tokens.has(id)) return;
      const token = Symbol();
      const started = now();
      tokens.set(id, token);
      active.set(token, started);
      if (active.size === PRESSURE_THRESHOLD) lastProgress = started;
      highWater = Math.max(highWater, active.size);
    }

    function end(id) {
      if (!active.delete(tokens.get(id))) return;
      tokens.delete(id);
      lastProgress = now();
    }

    function dispose() {
      if (disposed) return;
      disposed = true;
      for (const id of tokens.keys()) end(id);
      for (const close of children) close();
      parent?.delete(dispose);
    }
    parent?.add(dispose);

    function connect(port) {
      if (disposed) { port.close(); return; }
      const remote = scope();
      const disconnect = () => {
        children.delete(disconnect);
        remote.dispose();
        port.onmessage = null;
        port.close();
      };
      children.add(disconnect);
      port.onmessage = ({data}) => {
        if (data?.type === 'dispose') { disconnect(); return; }
        if (!Number.isSafeInteger(data?.id)) return;
        if (data.type === 'begin') remote.begin(data.id);
        if (data.type === 'end') remote.end(data.id);
      };
      port.addEventListener('close', disconnect, {once: true});
      return disconnect;
    }

    return {
      begin, end, dispose, connect,
      nextId: () => nextJsId--,
      child() {
        const child = scope(children);
        if (disposed) child.dispose();
        return child;
      },
      snapshot() {
        const time = now();
        return {
          active: active.size, highWater,
          oldestActiveMs: Math.floor(time - (active.values().next().value ?? time)),
          suspectedStarvation: active.size >= PRESSURE_THRESHOLD && time - lastProgress >= NO_PROGRESS_MS,
        };
      },
    };
  }
  return scope();
}

function corePortClient(port) {
  let nextJsId = -1;
  return {
    nextId: () => nextJsId--,
    begin: id => port.postMessage({type: 'begin', id}),
    end: id => port.postMessage({type: 'end', id}),
    dispose: () => port.close(),
  };
}

export function installCoreHttp() {
  if (globalThis.__dclCoreHttp) return globalThis.__dclCoreHttp;
  if (typeof DedicatedWorkerGlobalScope !== 'undefined' && self instanceof DedicatedWorkerGlobalScope) {
    const {port1, port2} = new MessageChannel();
    self.postMessage({__dclCoreHttp: true}, [port2]);
    globalThis.__dclCoreHttp = corePortClient(port1);
    // Closing the worker can discard pending reads without notifying the peer port.
    const close = self.close.bind(self);
    self.close = () => {
      try { port1.postMessage({type: 'dispose'}); } catch { /* diagnostics must not prevent shutdown */ }
      close();
    };
  } else {
    const host = createCoreHttpHost();
    globalThis.__dclCoreHttp = host;
    globalThis.addEventListener?.('pagehide', onPageHide);
  }
  return globalThis.__dclCoreHttp;
}

function onPageHide(event) {
  if (!event.persisted) disposeCoreHttp();
}

export function disposeCoreHttp() {
  globalThis.removeEventListener?.('pagehide', onPageHide);
  globalThis.__dclCoreHttp?.dispose?.();
  delete globalThis.__dclCoreHttp;
}

let shellHost;
export function installHudHttp(inShell) {
  try {
    const host = installCoreHttp();
    if (!inShell || host === shellHost) return;
    const {port1, port2} = new MessageChannel();
    host.connect(port1);
    window.parent.postMessage({type: 'bevy-shell:http'}, location.origin, [port2]);
    shellHost = host;
  } catch { /* HUD requests also work without shell reporting. */ }
}

// The shell verifies the sender's frame and origin before handing us this port.
export function connectShellHttp(port) {
  globalThis.__dclCoreHttp?.dispose();
  globalThis.__dclCoreHttp = corePortClient(port);
}

// Only the page attaches trusted engine workers. A recoverable worker error can leave
// live transfers, so retire them on termination or a closed port.
const attachedWorkers = new WeakSet();
export function attachCoreWorker(worker) {
  if (attachedWorkers.has(worker)) return;
  attachedWorkers.add(worker);
  let disconnect;
  const connect = event => {
    if (event.data?.__dclCoreHttp === true && event.ports[0]) {
      disconnect?.();
      try { disconnect = installCoreHttp().connect(event.ports[0]); } catch { event.ports[0].close(); }
    }
  };
  worker.addEventListener('message', connect);
  const terminate = worker.terminate.bind(worker);
  worker.terminate = () => {
    try {
      worker.removeEventListener('message', connect);
      disconnect?.();
    } finally { terminate(); }
  };
}

// wasm-bindgen calls these synchronously around Rust's own transport and read.
export function beginCoreHttp(id) {
  try { installCoreHttp().begin(id); } catch { /* diagnostics are best effort */ }
}
export function endCoreHttp(id) {
  try { globalThis.__dclCoreHttp?.end(id); } catch { /* reporter unavailable */ }
}

// Wrap one transfer whose owner already reads the complete response.
export async function observeCoreHttp(transfer) {
  let reporter, id;
  try {
    reporter = installCoreHttp();
    id = reporter.nextId();
    reporter.begin(id);
  } catch { /* transport also works without a reporter */ }
  try {
    return await transfer();
  } finally {
    try { reporter?.end(id); } catch { /* preserve the transport result */ }
  }
}

export function coreFetch(input, init, read) {
  return observeCoreHttp(async () => {
    const response = init === undefined ? await globalThis.fetch(input) : await globalThis.fetch(input, init);
    if (!read) return response; // The caller takes ownership of the original stream at headers.
    try {
      return await read(response);
    } finally {
      if (response.body && !response.body.locked) response.body.cancel().catch(() => {});
    }
  });
}

export function coreFetchJson(input, init, errorPrefix = 'HTTP request failed:') {
  return coreFetch(input, init, response => {
    if (!response.ok) throw new Error(`${errorPrefix} ${response.status}`);
    return response.json();
  });
}
