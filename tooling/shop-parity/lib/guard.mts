// Keep a process off the network: every TCP connection it opens must go to loopback. http, https,
// fetch (undici) and tls all connect through net.Socket, so one patch covers them. And keep other
// sites from driving a loopback server through the developer's browser.
import type { IncomingHttpHeaders } from "node:http";
import { Socket } from "node:net";

/**
 * Whether a loopback server serving `base` must refuse a request: one another site made
 * (`Sec-Fetch-Site` other than same-origin or none, or a foreign `Origin`), or one naming another host.
 * A page whose own name was rebound to 127.0.0.1 is same-origin to itself, but names that host in Host.
 */
export function foreignRequest(headers: IncomingHttpHeaders, base: URL): boolean {
  const site = headers["sec-fetch-site"];
  if (headers.host !== base.host) return true;
  return (site !== undefined && site !== "same-origin" && site !== "none") || (headers.origin !== undefined && headers.origin !== base.origin);
}

const LOOPBACK = new Set(["127.0.0.1", "::1", "localhost"]);

/** The host:port a Socket#connect call targets, or null for a local (IPC) path. */
function target(args: unknown[]): string | null {
  // net.connect normalizes its arguments to [options, listener] before calling Socket#connect.
  const first = Array.isArray(args[0]) ? (args[0] as unknown[])[0] : args[0];
  if (first && typeof first === "object") {
    const o = first as { path?: string; host?: string; port?: number | string };
    if (o.path) return null;
    return `${o.host ?? "localhost"}:${o.port}`;
  }
  if (typeof first === "string" && !/^\d+$/.test(first)) return null;
  return `${typeof args[1] === "string" ? args[1] : "localhost"}:${first}`;
}

/**
 * Refuse every TCP connection to a host that is not loopback; `onBlock` hears each refused host:port.
 * Returns the function that lifts the guard.
 */
export function guardLoopback(onBlock: (hostPort: string) => void): () => void {
  const connect = Socket.prototype.connect;
  Socket.prototype.connect = function (this: Socket, ...args: unknown[]) {
    const to = target(args);
    if (to !== null) {
      const host = to.slice(0, to.lastIndexOf(":")).replace(/^\[|\]$/g, "");
      if (!LOOPBACK.has(host)) {
        onBlock(to);
        process.nextTick(() => this.destroy(new Error(`blocked a connection to ${to}: only loopback is allowed`)));
        return this;
      }
    }
    return (connect as (...a: unknown[]) => Socket).apply(this, args);
  } as typeof connect;
  return () => {
    Socket.prototype.connect = connect;
  };
}
