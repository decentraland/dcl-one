export function isDevHost(): boolean {
  // The host alone decides, not the dev server: the node's public host in dev mode
  // offers what it offers in prod.
  if (typeof window === "undefined") return false;
  const host = window.location.hostname;
  return (
    host === "localhost" ||
    host === "127.0.0.1" ||
    host === "::1" ||
    host === "[::1]" ||
    host === "0.0.0.0" ||
    host.endsWith(".local")
  );
}
