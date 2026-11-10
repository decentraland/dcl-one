// Each sandbox worker shares five sends between fetch and signedFetch. Scene code shares this
// realm and can replace globals and built-in methods, so what the limit relies on is captured at
// load, before any scene code runs, and the queue is a linked list of plain objects.
const NativePromise = Promise;
const NativeRequest = Request;
const promiseThen = Function.prototype.call.bind(Promise.prototype.then);
const deleteHeader = Function.prototype.call.bind(Headers.prototype.delete);
const getter = (proto, name) =>
  Function.prototype.call.bind(Object.getOwnPropertyDescriptor(proto, name).get);
const requestHeaders = getter(Request.prototype, "headers");
const requestSignal = getter(Request.prototype, "signal");
const addListener = Function.prototype.call.bind(EventTarget.prototype.addEventListener);
const removeListener = Function.prototype.call.bind(EventTarget.prototype.removeEventListener);

export function createSceneFetch(nativeFetch) {
  let head = null;
  let tail = null;
  let active = 0;

  function next() {
    while (head !== null) {
      const entry = head;
      head = entry.next;
      if (head === null) tail = null;
      if (!entry.done) return entry.start();
    }
  }

  return function sceneFetch(input, init) {
    return new NativePromise((resolve, reject) => {
      // Snapshot caller-owned headers, body and options before waiting for a slot.
      const request = new NativeRequest(input, init);
      const signal = requestSignal(request);
      // The service worker answers X-IPFS requests from the client's content cache.
      deleteHeader(requestHeaders(request), "X-IPFS");
      // Scenes never send or store cookies and never read or fill the HTTP cache. Chromium
      // only exposes its local-network annotation on RequestInit. Passing a new init to fetch
      // also resets the referrer unless we supply it again.
      const addressSpace = init?.targetAddressSpace;
      const options = {
        credentials: "omit",
        cache: "no-store",
        referrer: request.referrer,
        referrerPolicy: request.referrerPolicy,
        ...(addressSpace === undefined ? {} : { targetAddressSpace: String(addressSpace) }),
      };
      const entry = { start, next: null, done: false };

      function cancelQueued() {
        entry.done = true;
        // Notify an unsent upload's producer without delaying the rejection.
        request.body?.cancel(signal.reason).catch(() => {});
        reject(signal.reason);
      }

      function start() {
        entry.done = true;
        removeListener(signal, "abort", cancelQueued);
        active++;
        // Browser fetch settles at headers. The caller owns the response stream.
        const settle = () => {
          active--;
          next();
        };
        let sent;
        try {
          sent = nativeFetch(request, options);
        } catch (error) {
          settle();
          reject(error);
          return;
        }
        promiseThen(
          sent,
          (response) => {
            settle();
            resolve(response);
          },
          (error) => {
            settle();
            reject(error);
          },
        );
      }

      if (signal.aborted) cancelQueued();
      else if (active < 5) start();
      else {
        if (tail === null) head = entry;
        else tail.next = entry;
        tail = entry;
        addListener(signal, "abort", cancelQueued, { once: true });
      }
    });
  };
}
