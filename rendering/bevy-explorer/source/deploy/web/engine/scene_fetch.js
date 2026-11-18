// Each sandbox worker shares five sends between fetch and signedFetch, and sends what scenes
// ask of third-party services through the page's own node (createSceneRoutes). Scene code shares
// this realm and can replace globals, built-in methods and Object.prototype, so everything a send
// relies on is captured at load, before any scene code runs, every init has a null prototype, and
// the queue is a linked list of plain objects.

const NativePromise = Promise;
const NativeRequest = Request;
const NativeURL = URL;
const call = Function.prototype.call;
const bindCall = (fn) => call.bind(fn);
const promiseThen = bindCall(Promise.prototype.then);
const defineProperty = Object.defineProperty;
const deleteHeader = bindCall(Headers.prototype.delete);
const mapGet = bindCall(Map.prototype.get);
const getter = (proto, name) => bindCall(Object.getOwnPropertyDescriptor(proto, name).get);
const requestHeaders = getter(Request.prototype, "headers");
const requestSignal = getter(Request.prototype, "signal");
const requestMethod = getter(Request.prototype, "method");
const requestUrl = getter(Request.prototype, "url");
const requestReferrer = getter(Request.prototype, "referrer");
const requestReferrerPolicy = getter(Request.prototype, "referrerPolicy");
const requestArrayBuffer = bindCall(Request.prototype.arrayBuffer);
const urlProtocol = getter(URL.prototype, "protocol");
const urlHost = getter(URL.prototype, "host");
const urlPathname = getter(URL.prototype, "pathname");
const urlSearch = getter(URL.prototype, "search");
const addListener = bindCall(EventTarget.prototype.addEventListener);
const removeListener = bindCall(EventTarget.prototype.removeEventListener);
const ONCE = { __proto__: null, once: true };

// Runs a callback when a promise settles without reading its `constructor` or Symbol.species,
// which scene code can replace; the promise `then` returns is never used.
function whenSettled(promise, onFulfilled, onRejected) {
  defineProperty(promise, "constructor", { __proto__: null, value: undefined });
  promiseThen(promise, onFulfilled, onRejected);
}

// A node that fails or cannot be reached is asked once a minute, not at each spawn of a worker: the
// workers spawned meanwhile send direct.
const ROUTES_RETRY_MS = 60_000;

// The node's routing map, `GET /_scene` (services/catalyst/src/scene_proxy.rs): host to a path
// prefix on this origin. The page reads it once and hands it to every worker it spawns
// (sandbox_host.js). A node that answers without a map has none: the page keeps an empty one and
// does not ask again.
export function createSceneRoutes(nativeFetch, origin = self.location.origin) {
  let routes = null;
  let pending = null;
  let failedAt = 0;
  return function sceneRoutes() {
    if (routes) return Promise.resolve(routes);
    if (pending) return pending;
    if (failedAt && Date.now() - failedAt < ROUTES_RETRY_MS) return Promise.resolve(null);
    pending = nativeFetch(`${origin}/_scene`, { __proto__: null, credentials: "omit" })
      .then(async (response) => {
        // An answer that is not read as a map: its body is dropped, so it holds no connection.
        if (!response.ok) await response.body?.cancel().catch(() => {});
        if (response.status >= 500) throw new Error(`routing map: ${response.status}`);
        const body = response.ok ? await response.json().catch(() => null) : null;
        routes = new Map(Object.entries(body?.routes ?? {}));
        return routes;
      })
      .catch(() => { failedAt = Date.now(); return null; })
      .finally(() => { pending = null; });
    return pending;
  };
}

// `https://<host>/<path>` becomes `<origin><prefix>/<path>` when the map names the host, for the
// methods the proxy forwards; anything else stays as it is (null).
function routedUrl(request, routes, origin) {
  const method = requestMethod(request);
  if (!routes || (method !== "GET" && method !== "HEAD" && method !== "POST")) return null;
  let url;
  try {
    url = new NativeURL(requestUrl(request));
  } catch {
    return null;
  }
  const protocol = urlProtocol(url);
  if (protocol !== "https:" && protocol !== "http:") return null;
  const prefix = mapGet(routes, urlHost(url));
  if (typeof prefix !== "string") return null;
  return `${origin}${prefix}${urlPathname(url)}${urlSearch(url)}`;
}

// The request re-addressed to `target`, with `body` in place of a POST's stream, which cannot be
// re-addressed. Credentials are omitted, so nothing a scene sends carries the page's session.
function readdress(request, target, body) {
  return new NativeRequest(target, {
    __proto__: null,
    method: requestMethod(request),
    headers: requestHeaders(request),
    body,
    credentials: "omit",
    cache: "no-store",
    referrerPolicy: "no-referrer",
    signal: requestSignal(request),
  });
}

// The routed request, a promise of it for a POST (once its body is read), or null.
export function routeSceneRequest(request, routes, origin = self.location.origin) {
  const target = routedUrl(request, routes, origin);
  if (target === null) return null;
  if (requestMethod(request) !== "POST") return readdress(request, target, undefined);
  return requestArrayBuffer(request).then((body) => readdress(request, target, body));
}

// `sceneRoutes` returns the current map, or null while there is none.
export function createSceneFetch(nativeFetch, sceneRoutes = () => null, origin = self.location.origin) {
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

  // What is sent is built here and only here, whatever the route made of the request. Scenes
  // never send or store cookies, never read or fill the HTTP cache, and never reach the service
  // worker's content cache (X-IPFS). Passing an init to fetch resets the referrer unless it is
  // given again, and Chromium only reads its local-network annotation from fetch's init.
  function send(request, addressSpace) {
    const init = {
      __proto__: null,
      credentials: "omit",
      cache: "no-store",
      referrer: requestReferrer(request),
      referrerPolicy: requestReferrerPolicy(request),
    };
    if (addressSpace !== undefined) init.targetAddressSpace = addressSpace;
    const out = new NativeRequest(request, init);
    deleteHeader(requestHeaders(out), "X-IPFS");
    return nativeFetch(out, init);
  }

  return function sceneFetch(input, init) {
    return new NativePromise((resolve, reject) => {
      // Snapshot caller-owned headers, body and options before waiting for a slot.
      const request = new NativeRequest(input, init);
      const signal = requestSignal(request);
      const addressSpace = init?.targetAddressSpace;
      const space = addressSpace === undefined ? undefined : `${addressSpace}`;
      const entry = { __proto__: null, start, next: null, done: false };

      function cancelQueued() {
        entry.done = true;
        // Notify an unsent upload's producer without delaying the rejection.
        request.body?.cancel(signal.reason).catch(() => {});
        reject(signal.reason);
      }

      function start() {
        entry.done = true;
        active++;
        // Browser fetch settles at headers. The caller owns the response stream. The caller is
        // answered before the slot passes on, and a slot is given back once.
        let settled = false;
        const finish = (settle, value) => {
          if (settled) return;
          settled = true;
          settle(value);
          active--;
          next();
        };
        const fail = (error) => finish(reject, error);
        const dispatch = (out) => whenSettled(out, (response) => finish(resolve, response), fail);
        try {
          removeListener(signal, "abort", cancelQueued);
          // A send that waits for nothing leaves the moment its slot starts.
          const target = routedUrl(request, sceneRoutes(), origin);
          if (target === null) dispatch(send(request, space));
          else if (requestMethod(request) !== "POST") dispatch(send(readdress(request, target, undefined)));
          else {
            whenSettled(
              requestArrayBuffer(request),
              (body) => {
                try {
                  dispatch(send(readdress(request, target, body)));
                } catch (error) {
                  fail(error);
                }
              },
              fail,
            );
          }
        } catch (error) {
          fail(error);
        }
      }

      if (signal.aborted) cancelQueued();
      else if (active < 5) start();
      else {
        if (tail === null) head = entry;
        else tail.next = entry;
        tail = entry;
        addListener(signal, "abort", cancelQueued, ONCE);
      }
    });
  };
}
