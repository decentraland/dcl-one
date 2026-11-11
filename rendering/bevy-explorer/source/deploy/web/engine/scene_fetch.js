// Each sandbox worker shares five sends between fetch and signedFetch, and sends what scenes
// ask of third-party services through the page's own node (createSceneRoutes).

const ROUTED_METHODS = new Set(["GET", "HEAD", "POST"]);
const ROUTES_RETRY_MS = 60_000;

// The node's routing map, `GET /_scene` (services/catalyst/src/scene_proxy.rs): host to a path
// prefix on this origin. The node's own places and marketplace routes stand in for those services;
// the other hosts go through its allow-listed proxy, as they answer no CORS preflight. Read once
// per worker; while it cannot be read, scene requests go out direct, and it is tried again a minute
// later.
export function createSceneRoutes(nativeFetch, origin = self.location.origin) {
  let routes = null;
  let pending = null;
  let failedAt = 0;
  return function sceneRoutes() {
    if (routes) return Promise.resolve(routes);
    if (pending) return pending;
    if (failedAt && Date.now() - failedAt < ROUTES_RETRY_MS) return Promise.resolve(null);
    pending = nativeFetch(`${origin}/_scene`, { credentials: "omit" })
      .then(async (response) => {
        if (!response.ok) throw new Error(`routing map: ${response.status}`);
        const body = await response.json();
        routes = new Map(Object.entries(body.routes ?? {}));
        return routes;
      })
      .catch(() => { failedAt = Date.now(); return null; })
      .finally(() => { pending = null; });
    return pending;
  };
}

// `https://<host>/<path>` becomes `<origin><prefix>/<path>` when the map names the host, for the
// methods the proxy forwards; anything else stays as it is. The body is read here: a request with a
// stream body cannot be re-addressed. Credentials are omitted, so nothing a scene sends carries the
// page's session; the proxy forwards an allow-list of headers (the signed-fetch identity headers
// among them) and nothing else.
export async function routeSceneRequest(request, routes, origin = self.location.origin) {
  if (!routes || !ROUTED_METHODS.has(request.method)) return null;
  let url;
  try {
    url = new URL(request.url);
  } catch {
    return null;
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") return null;
  const prefix = routes.get(url.host);
  if (prefix === undefined) return null;
  const body = request.method === "GET" || request.method === "HEAD" ? undefined : await request.arrayBuffer();
  return new Request(`${origin}${prefix}${url.pathname}${url.search}`, {
    method: request.method,
    headers: request.headers,
    body,
    credentials: "omit",
    referrerPolicy: "no-referrer",
    signal: request.signal,
  });
}

export function createSceneFetch(nativeFetch, sceneRoutes = createSceneRoutes(nativeFetch)) {
  const queue = new Set();
  let active = 0;

  return function sceneFetch(input, init) {
    return new Promise((resolve, reject) => {
      // Snapshot caller-owned headers, body and options before waiting for a slot.
      const request = new Request(input, init);
      const { signal } = request;
      // Chromium only exposes its local-network annotation on RequestInit. Passing
      // a new init to fetch also resets the referrer unless we supply it again.
      const addressSpace = init?.targetAddressSpace;
      const options = addressSpace === undefined ? undefined : {
        targetAddressSpace: String(addressSpace),
        referrer: request.referrer,
        referrerPolicy: request.referrerPolicy,
      };

      function cancelQueued() {
        queue.delete(start);
        // Notify an unsent upload's producer without delaying the rejection.
        request.body?.cancel(signal.reason).catch(() => {});
        reject(signal.reason);
      }

      async function start() {
        queue.delete(start);
        signal.removeEventListener("abort", cancelQueued);
        active++;
        try {
          const routed = await routeSceneRequest(request, await sceneRoutes());
          resolve(await nativeFetch(routed ?? request, routed ? undefined : options));
        } catch (error) {
          reject(error);
        } finally {
          // Browser fetch settles at headers. The caller owns the response stream.
          active--;
          queue.values().next().value?.();
        }
      }

      if (signal.aborted) cancelQueued();
      else if (active < 5) start();
      else {
        queue.add(start);
        signal.addEventListener("abort", cancelQueued, { once: true });
      }
    });
  };
}
