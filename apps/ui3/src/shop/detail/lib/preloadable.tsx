import { Component, lazy, useState, type ComponentType, type ReactNode } from "react";

export type Preloadable<P extends object> = ComponentType<P> & { preload(): Promise<unknown> };

/**
 * A component whose code loads on demand, as lazy(), but which renders at once when `preload` has resolved. lazy()
 * suspends on its first render even when the code is in: React then commits the Suspense fallback and holds the
 * reveal back for its fallback throttle.
 *
 * A failed load is not kept. `preload` settles without rejecting, and the next mount loads the code again instead of
 * rethrowing the failure that React's lazy() keeps. The mount that failed throws to the nearest error boundary.
 */
export function preloadable<P extends object>(load: () => Promise<{ default: ComponentType<P> }>): Preloadable<P> {
  let loaded: ComponentType<P> | null = null;
  let pending: Promise<{ default: ComponentType<P> }> | null = null;
  let Lazy: ComponentType<P> | null = null;
  const loadOnce = () =>
    (pending ??= load().then(
      (module) => {
        loaded = module.default;
        return module;
      },
      (err: unknown) => {
        pending = null;
        Lazy = null;
        throw err;
      },
    ));
  function Part(props: P) {
    // Fixed for the mount: going from the lazy wrapper to the loaded component would remount the part.
    const [Component] = useState(() => loaded ?? (Lazy ??= lazy(loadOnce) as ComponentType<P>));
    return <Component {...props} />;
  }
  return Object.assign(Part, { preload: () => loadOnce().catch(() => undefined) });
}

/**
 * Around on-demand dialogs: one that throws (its code did not load, say) is closed through `onClose`, which must
 * unmount it, and the page stays. The next open loads its code again.
 */
export class DialogBoundary extends Component<{ onClose: () => void; children: ReactNode }, { failed: boolean }> {
  override state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  override componentDidCatch(error: Error) {
    console.error("[shop] a dialog failed", error);
    this.props.onClose();
    this.setState({ failed: false });
  }
  override render() {
    return this.state.failed ? null : this.props.children;
  }
}
