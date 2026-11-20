import { lazy, useState, type ComponentType } from "react";

export type Preloadable<P extends object> = ComponentType<P> & { preload(): Promise<unknown> };

/**
 * A component whose code loads on demand, as lazy(), but which renders at once when `preload` has resolved. lazy()
 * suspends on its first render even when the code is in: React then commits the Suspense fallback and holds the
 * reveal back for its fallback throttle.
 */
export function preloadable<P extends object>(load: () => Promise<{ default: ComponentType<P> }>): Preloadable<P> {
  let loaded: ComponentType<P> | null = null;
  const preload = () =>
    load().then((module) => {
      loaded = module.default;
      return module;
    });
  const Lazy = lazy(preload) as ComponentType<P>;
  function Part(props: P) {
    // Fixed for the mount: going from the lazy wrapper to the loaded component would remount the part.
    const [Component] = useState(() => loaded ?? Lazy);
    return <Component {...props} />;
  }
  return Object.assign(Part, { preload });
}
