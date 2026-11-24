import { PAGE_DIR } from './publicUrl'
import { serviceUrl } from './baseDomain'

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
  if (v.toLowerCase() === 'none') return true
  if (v === normalise(SYSTEM_SCENE)) return true

  const lower = v.toLowerCase()
  if (TRUSTED_WORLDS.includes(lower)) return true
  if (lower.startsWith(WORLDS_PREFIX)) return TRUSTED_WORLDS.includes(lower.slice(WORLDS_PREFIX.length))

  try {
    if (LOOPBACK.has(new URL(v).hostname.toLowerCase())) return true
  } catch {
  }
  return false
}
