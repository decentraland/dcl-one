// Each sandbox worker shares five sends between fetch and signedFetch.
export function createSceneFetch(nativeFetch) {
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
          resolve(await nativeFetch(request, options));
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
