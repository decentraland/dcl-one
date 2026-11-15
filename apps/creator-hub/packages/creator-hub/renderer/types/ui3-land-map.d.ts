// The ui3 LAND map imports ui3's HTTP client, and that client reads the Explorer bridge from the
// window. Creator Hub has no bridge. This is the part of its type that the client reads, so the
// renderer does not compile the Explorer overlay.
interface Window {
  dclBridge?: {
    send?: (...args: unknown[]) => unknown;
    onState?: (listener: (push: unknown) => void) => () => void;
  };
}
