import { useEffect } from "react";

import type { Route } from "./+types/play";

// The web Explorer's shell page: the top-level document that keeps the sign-in key and embeds
// the HUD + engine document (/play/app.html) in an iframe the browser isolates in a process of its
// own. The logic is the upstream shell (apps/ui3/src/play/shell); this page is deliberately NOT
// cross-origin isolated, the service worker at /play/service_worker.js strips any host COEP.
// play_.app[.]html.tsx (the app) is a sibling route, not a child: the shell never runs on /play/app.html.
export function meta(_args: Route.MetaArgs) {
  return [
    { title: "Decentraland" },
    {
      name: "description",
      content:
        "Explore Decentraland, a social virtual world. Jump in from your browser: events, games, and places built by the community.",
    },
    { name: "theme-color", content: "#2c1442" },
    { property: "og:title", content: "Decentraland" },
    { property: "og:description", content: "Explore a social virtual world from your browser." },
  ];
}

export function links() {
  return [
    { rel: "icon", type: "image/png", href: "/play/favicon/favicon-96x96.png", sizes: "96x96" },
    { rel: "icon", type: "image/svg+xml", href: "/play/favicon/favicon.svg" },
    { rel: "apple-touch-icon", sizes: "180x180", href: "/play/favicon/apple-touch-icon.png" },
    { rel: "manifest", href: "/play/favicon/site.webmanifest" },
  ];
}

export function headers(): Record<string, string> {
  return { "Cache-Control": "private, no-store" };
}

const SHELL_STYLE = `
html, body { margin: 0; height: 100%; overflow: hidden; background: #54098a; }
body > iframe { position: fixed; inset: 0; width: 100%; height: 100%; border: 0; }
body > p { margin: 40vh 24px 0; color: #fff; font: 16px system-ui, sans-serif; text-align: center; }
`;

export default function PlayShell() {
  useEffect(() => {
    void import("@ui/play/shell/mount").then((m) => m.mountPlayShell());
  }, []);
  return <style>{SHELL_STYLE}</style>;
}
