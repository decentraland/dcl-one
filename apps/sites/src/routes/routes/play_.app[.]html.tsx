import { useEffect, useRef } from "react";

import type { Route } from "./+types/play_.app[.]html";

// The web Explorer's app document (upstream app.html): the React HUD (apps/ui3/src/play) with the
// wasm engine's canvas behind it, in this same document. The engine needs SharedArrayBuffer and
// WebGPU, so this document is cross-origin isolated the way upstream isolates app.html:
// Document-Isolation-Policy isolate-and-credentialless (scene assets come from catalysts that send
// no CORP header), never COEP. The engine's scene-server frame (engine/headless.html) carries the
// same policy so this document can script it; the engine's service worker keys both on these exact
// paths. Opened outside the shell (/play) the HUD redirects there.
export function meta(_args: Route.MetaArgs) {
  return [{ title: "Decentraland" }, { name: "theme-color", content: "#2c1442" }];
}

export function links() {
  return [
    { rel: "icon", type: "image/png", href: "/play/favicon/favicon-96x96.png", sizes: "96x96" },
    { rel: "icon", type: "image/svg+xml", href: "/play/favicon/favicon.svg" },
    { rel: "prefetch", as: "image", href: "/play/assets/login/gradient-background.webp" },
  ];
}

export function headers(): Record<string, string> {
  return {
    "Cache-Control": "private, no-store",
    "Cross-Origin-Opener-Policy": "same-origin",
    "Document-Isolation-Policy": "isolate-and-credentialless",
    "Cross-Origin-Resource-Policy": "cross-origin",
  };
}

// Instant branded first paint before the HUD bundle mounts (mirrors the upstream app.html): the
// splash, the animated logo on the purple vignette, which the HUD's own splash (StartupSplash) continues.
const SPLASH_STYLE = `
html, body { margin: 0; background: #4c0080; }
#play-boot-splash { position: fixed; inset: 0; display: flex; align-items: center; justify-content: center;
  background: radial-gradient(ellipse 100% 100% at 50% 50%, #bf00ff 16.7%, #4c0080 66.7%); }
#play-boot-splash img { width: clamp(74.4px, min(6.458vw, 11.481vh), 161.2px); height: clamp(74.4px, min(6.458vw, 11.481vh), 161.2px); }
`;

export default function PlayApp() {
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const container = root.current;
    if (!container) return;
    let unmount: (() => void) | undefined;
    let cancelled = false;
    void import("@ui/play/mount").then(async (m) => {
      if (cancelled) return;
      unmount = await m.mountPlayApp(container);
    });
    return () => {
      cancelled = true;
      unmount?.();
    };
  }, []);
  return (
    <>
      <style>{SPLASH_STYLE}</style>
      <div id="root" ref={root}>
        <div id="play-boot-splash">
          <img src="/play/assets/splash-logo.webp" alt="" />
        </div>
      </div>
    </>
  );
}
