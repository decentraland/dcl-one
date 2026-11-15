// The service worker serves the sandbox script with a Content-Security-Policy that loads no
// module and no font (service_worker.js). A worker started outside it has none, so scene code
// could load both with the origin's cookies.
const EMPTY_MODULE = "data:text/javascript,";

export function sandboxPolicyInForce() {
  // A variable, so the bundler leaves the import to the browser.
  return import(EMPTY_MODULE).then(
    () => false,
    () => true,
  );
}
