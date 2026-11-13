// Copyright 2018-2026 the Deno authors. MIT license.
// Replaces deno_fetch 0.284.0 22_http_client.js: scenes cannot create HTTP clients, so a
// request's client is always the default one.

(function () {
class HttpClient {
  constructor() {
    throw new TypeError("Illegal constructor");
  }
}
const HttpClientPrototype = HttpClient.prototype;

return { HttpClient, HttpClientPrototype };
})();
