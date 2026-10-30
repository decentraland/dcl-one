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

// Instant branded first paint before the HUD bundle mounts (mirrors the upstream app.html).
const SPLASH_STYLE = `
html, body { margin: 0; background: #54098a; }
#play-boot-splash { position: fixed; inset: 0; display: flex; flex-direction: column; align-items: center;
  justify-content: center; gap: 20px; font-family: system-ui, -apple-system, sans-serif; color: #fff;
  background: radial-gradient(circle at 66% 38%, #b528d8 0%, #8016b8 44%, #54098a 100%); }
#play-boot-splash img { width: 88px; height: 88px; animation: play-boot-pulse 1.6s ease-in-out infinite; }
#play-boot-splash h1 { margin: 0; font-size: 1.5rem; font-weight: 600; letter-spacing: 0.01em; }
@keyframes play-boot-pulse { 0%, 100% { transform: scale(1); opacity: .85; } 50% { transform: scale(1.08); opacity: 1; } }
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
          <img src="/play/assets/logo.png" alt="" />
          <h1>Decentraland</h1>
        </div>
      </div>
    </>
  );
}
