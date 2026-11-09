// The only globals the core may use beyond ES: the ones every Shop host has (browsers, the Explorer's
// CEF and node), each as narrow as its use. library/shop/tsconfig.json compiles the core with these and
// ES alone, so anything else a host offers (window, document, fetch, storage, WebSocket...) fails to
// compile. Hosts that type-check the core with their own lib (apps/ui3) do not include this file.

declare var console: { warn(...data: unknown[]): void };
declare var TextEncoder: { new (): { encode(input?: string): Uint8Array } };
// WHATWG URL: the client serializes a query with URLSearchParams (its encoding is the wire format) and signs the path
// URL parses. Never fetch: the host's transport makes every request.
declare var URL: { new (url: string, base?: string): { readonly pathname: string } };
declare var URLSearchParams: {
  new (): { append(name: string, value: string): void; set(name: string, value: string): void; toString(): string };
};
// A caller's signal, only ever handed on to the host's transport as it came: never built, read for more or replaced.
declare interface AbortSignal {
  readonly aborted: boolean;
}
