// The only globals the core may use beyond ES: the ones every Shop host has (browsers, the Explorer's
// CEF and node), each as narrow as its use. library/shop/tsconfig.json compiles the core with these and
// ES alone, so anything else a host offers (window, document, fetch, storage, WebSocket...) fails to
// compile. Hosts that type-check the core with their own lib (apps/ui3) do not include this file.

declare var console: { warn(...data: unknown[]): void };
declare var TextEncoder: { new (): { encode(input?: string): Uint8Array } };
