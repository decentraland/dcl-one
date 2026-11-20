import { readdirSync } from "node:fs";

export type RouteAuth = "both" | "out";

export type Route = {
  path: string;
  auth: RouteAuth;
  allowConsole?: RegExp[];
};

const AUTH_PROBE = /Failed to load resource.*40[13]/;
// Headless chromium exposes the WebGPU API without a real adapter, so the
// engine passes the browser-gate then panics at wgpu adapter selection.
// Real browsers with GPUs are unaffected; keep this ONLY on engine routes.
const HEADLESS_NO_GPU = [
  /Unable to find a GPU/,
  /panicked at .*bevy_render/,
  /RuntimeError: unreachable/,
];

export const ROUTES: Route[] = [
  { path: "/create", auth: "both" },
  { path: "/create/collections", auth: "out", allowConsole: [AUTH_PROBE] },
  { path: "/create/curation", auth: "out", allowConsole: [AUTH_PROBE] },
  { path: "/creator-hub/claim-name", auth: "both", allowConsole: [AUTH_PROBE] },

  // The Shop (ui3/src/shop); every old /marketplace page redirects to one of these.
  { path: "/shop/overview", auth: "both", allowConsole: [AUTH_PROBE] },
  { path: "/shop/items", auth: "both", allowConsole: [AUTH_PROBE] },
  { path: "/shop/names", auth: "out", allowConsole: [AUTH_PROBE] },
  { path: "/shop/lands", auth: "out", allowConsole: [AUTH_PROBE] },
  { path: "/shop/credits", auth: "out", allowConsole: [AUTH_PROBE] },
  { path: "/shop/my-favorites", auth: "out", allowConsole: [AUTH_PROBE] },
  { path: "/shop/cart", auth: "both", allowConsole: [AUTH_PROBE] },

  { path: "/", auth: "out" },
  // The web Explorer: the shell page and the HUD on the fake bridge (no engine, no WebGPU).
  { path: "/play", auth: "out" },
  { path: "/play/app.html?mock=1", auth: "out" },
  { path: "/shop", auth: "out" },
  { path: "/marketplace", auth: "out" },
  { path: "/discover", auth: "out" },
  { path: "/events", auth: "out" },
  { path: "/places", auth: "out" },
  { path: "/shop/my-items", auth: "both", allowConsole: [AUTH_PROBE] },
  { path: "/blog", auth: "out" },
  { path: "/docs", auth: "out" },
  { path: "/seasons", auth: "out" },
  { path: "/privacy", auth: "out" },
  { path: "/terms", auth: "out" },
  { path: "/content", auth: "out" },
  { path: "/content-policy", auth: "out" },
  { path: "/credits-terms", auth: "out" },
  { path: "/creator-fund-terms", auth: "out" },
  { path: "/ethics", auth: "out" },
];

// The HUD lane (`smoke.mts --hud`) walks every panel the overlay registers. Same
// rule as ui3's router.tsx -- `panels/*.route.{jsx,tsx}`, id = lowercased file
// name -- so a new panel is covered the day its route file lands.
export function hudPanels(): string[] {
  const ext = /\.route\.[jt]sx$/;
  return readdirSync(new URL("../../../ui3/src/app/panels/", import.meta.url))
    .filter((f) => ext.test(f))
    .map((f) => f.replace(ext, "").toLowerCase())
    .sort();
}

// Engine-page noise that is the host or a tracked defect elsewhere, not a HUD
// regression: anything logged from a worker (the HUD is main-thread React; the
// workers are the engine and the sandboxes running whatever scene is deployed
// at the spawn parcel), the wasm world failing to render or tripping its
// watchdog, the lobby avatar preview, the comms preview socket flapping, asset
// 404s.
export const HUD_ALLOW: RegExp[] = [
  /^worker: /,
  ...HEADLESS_NO_GPU,
  /no frames for|world crashed|out of memory|wasm/i,
  /wearable-preview/i,
  /websocket|livekit|comms/i,
  /Failed to load resource/i,
];
