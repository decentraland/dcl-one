// Which super-user scene the engine boots, and whether a link is allowed to choose it.
//
// `?systemScene=` substitutes the scene that owns the UI, and that scene is trusted: permissions.rs
// short-circuits every permission check for it and it gets the whole SystemApi. So a crafted link
// on our own origin is a complete takeover of the session — no sandbox escape needed, just a click.
// Anything not recognised below gets the interstitial (lib/launchGate.ts →
// features/gate/UntrustedLaunchGate). What to trust is THIS front-end's call — the editor app,
// for one, trusts its editor scene.

import { PAGE_DIR } from './publicUrl'
import { serviceUrl } from './baseDomain'
import { isEditorScene } from './editorEmbed'

// Sites exposes our bundled bridge realm with this deployment's absolute content URLs.
// The compiled files remain under /play/bridge-scene/static; the realm endpoint adapts
// only the standalone export's baseUrl, without changing the scene or engine bytes.
export const SYSTEM_SCENE = new URL('bridge', PAGE_DIR).href

// Worlds we ship as first-party UI scenes. Accepted bare or as a full worlds-content-server url —
// the engine's ipfs layer expands `name.dcl.eth` into the latter, and boot.js reverses it for the
// address bar, so both spellings reach users.
const TRUSTED_WORLDS = ['tortilla.dcl.eth', 'sceneviewer.dcl.eth']
const WORLDS_PREFIX = `${serviceUrl('worldsServer')}/world/`

// A scene served from the developer's own machine. `sdk-commands start` is the whole point of the
// parameter, and a link can't make someone else's machine serve it.
const LOOPBACK = new Set(['localhost', '127.0.0.1', '[::1]'])

function normalise(value: string): string {
  return value.trim().replace(/\/+$/, '')
}

export function isTrustedSystemScene(value: string): boolean {
  const v = normalise(value)
  if (v === '') return true
  // 'none' runs NO ui scene — strictly less privilege than the default, never worth warning about.
  if (v.toLowerCase() === 'none') return true
  if (v === normalise(SYSTEM_SCENE)) return true
  // the scene editor's own scene, served by this deployment next to /play
  if (isEditorScene(v)) return true

  // Exact matches only. A suffix/`includes` test would accept `tortilla.dcl.eth.example.com`, and a
  // prefix test on the worlds url would accept `…/world/tortilla.dcl.eth/../../elsewhere`.
  const lower = v.toLowerCase()
  if (TRUSTED_WORLDS.includes(lower)) return true
  if (lower.startsWith(WORLDS_PREFIX)) return TRUSTED_WORLDS.includes(lower.slice(WORLDS_PREFIX.length))

  try {
    if (LOOPBACK.has(new URL(v).hostname.toLowerCase())) return true
  } catch {
    // not an absolute url — a bare path can't be vouched for, so it falls through to untrusted
  }
  return false
}
