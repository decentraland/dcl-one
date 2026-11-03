// Each sandbox worker owns one queue. Capture native operations at module startup,
// before scene code can replace globals or observe a continuation that releases a slot.
const NativeRequest = Request;
const NativePromise = Promise;
const NativeController = AbortController;
const NativeException = DOMException;
const defineProperty = Object.defineProperty;
const uncurry = fn => Function.call.bind(fn);
const getter = (type, key) => uncurry(Object.getOwnPropertyDescriptor(type.prototype, key).get);
const requestSignal = getter(Request, "signal");
const requestBody = getter(Request, "body");
const cancelBody = uncurry(ReadableStream.prototype.cancel);
const referrer = getter(Request, "referrer");
const referrerPolicy = getter(Request, "referrerPolicy");
const controllerSignal = getter(AbortController, "signal");
const abort = uncurry(AbortController.prototype.abort);
const aborted = getter(AbortSignal, "aborted");
const reason = getter(AbortSignal, "reason");
const combine = AbortSignal.any.bind(AbortSignal);
const listen = uncurry(EventTarget.prototype.addEventListener);
const unlisten = uncurry(EventTarget.prototype.removeEventListener);
const schedule = globalThis.setTimeout.bind(globalThis);
const clear = globalThis.clearTimeout.bind(globalThis);
const iterator = Symbol.iterator;
const next = uncurry(Object.getPrototypeOf(new Set().values()).next);

async function cancelUpload(request, error) {
  const body = requestBody(request);
  if (body) {
    // Cancellation also notifies the guest stream producer. Its own rejection
    // must not replace the reason Fetch rejected the request.
    try { await cancelBody(body, error); } catch {}
  }
}

export function createSceneFetch(nativeFetch, isServer, timeoutMs = isServer ? 15_000 : 30_000) {
  const limit = isServer ? 32 : 5;
  const lifetime = new NativeController();
  const lifetimeSignal = controllerSignal(lifetime);
  const queue = new Set();
  const enqueue = queue.add.bind(queue);
  const dequeue = queue.delete.bind(queue);
  const values = queue.values.bind(queue);
  let outstanding = 0;
  let active = 0;

  function fetch(input, init) {
    return new NativePromise((resolve, reject) => {
      if (aborted(lifetimeSignal)) throw reason(lifetimeSignal);
      // Request snapshots the caller's URL, headers and body before a queued send.
      // Chromium's local network annotation is only observable on RequestInit.
      const request = new NativeRequest(input, init);
      const addressSpace = init?.targetAddressSpace;
      const targetAddressSpace = addressSpace === undefined ? undefined : `${addressSpace}`;
      const deadline = new NativeController();
      // Web IDL reads a sequence via its iterator. A changed Array.prototype must
      // not let a scene remove the timeout from a live browser transfer.
      const signals = [requestSignal(request), controllerSignal(deadline), lifetimeSignal];
      const signal = combine({ [iterator]() {
        let i = 0;
        return { next: () => i < 3 ? {done: false, value: signals[i++]} : {done: true} };
      } });
      function rejectUnsent(error) {
        cancelUpload(request, error);
        reject(error);
      }
      if (aborted(signal)) return rejectUnsent(reason(signal));
      if (outstanding >= 32) {
        return rejectUnsent(new NativeException("Too many outstanding scene fetches", "QuotaExceededError"));
      }
      outstanding++;

      const timer = schedule(() => abort(deadline, new NativeException("Scene fetch timed out", "TimeoutError")), timeoutMs);
      function cancelQueued() {
        dequeue(start);
        clear(timer);
        outstanding--;
        rejectUnsent(reason(signal));
      }
      async function start() {
        dequeue(start);
        unlisten(signal, "abort", cancelQueued);
        active++;
        try {
          const headers = nativeFetch(request, {
            __proto__: null, signal, targetAddressSpace,
            referrer: referrer(request), referrerPolicy: referrerPolicy(request),
          });
          // Await the native promise directly even if the guest changed Promise.prototype.constructor.
          defineProperty(headers, "constructor", {__proto__: null, value: NativePromise});
          resolve(await headers);
        } catch (error) {
          reject(error);
        } finally {
          // The browser owns the response stream. Reading one body must never block
          // another caller waiting for headers. Its original signal remains connected.
          clear(timer);
          active--;
          outstanding--;
          next(values()).value?.();
        }
      }
      if (active < limit) start();
      else {
        enqueue(start);
        listen(signal, "abort", cancelQueued, {__proto__: null, once: true});
      }
    });
  }
  return { fetch, close: () => abort(lifetime) };
}

// The real global and its prototypes are both reachable from the guest.
export function installSceneFetch(scope, isServer, tearDown) {
  const client = createSceneFetch(scope.fetch.bind(scope), isServer);
  const defer = scope.queueMicrotask.bind(scope);
  const close = () => { client.close(); defer(tearDown); };
  for (let owner = scope; owner; owner = Object.getPrototypeOf(owner)) {
    if (Object.hasOwn(owner, "fetch")) owner.fetch = client.fetch;
    if (Object.hasOwn(owner, "close")) owner.close = close;
    delete owner.XMLHttpRequest;
    delete owner.EventSource;
  }
  return client;
}
