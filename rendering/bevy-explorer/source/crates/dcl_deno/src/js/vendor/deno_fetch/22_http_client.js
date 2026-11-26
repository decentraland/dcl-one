// Copyright 2018-2026 the Deno authors. MIT license.
// Replaces deno_fetch 0.284.0 22_http_client.js: the runtime provides no `createHttpClient`,
// so only `HttpClient` and its prototype remain, for 23_request.js.

(function () {
class HttpClient {
  constructor() {
    throw new TypeError("Illegal constructor");
  }
}
const HttpClientPrototype = HttpClient.prototype;

return { HttpClient, HttpClientPrototype };
})();
