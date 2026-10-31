// Keep a process off the network: every TCP connection it opens must go to loopback. http, https,
// fetch (undici) and tls all connect through net.Socket, so one patch covers them.
import { Socket } from "node:net";

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
