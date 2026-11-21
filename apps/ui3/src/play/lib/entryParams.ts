import { AUTH_RETURN_PARAM } from '@explorer-core/features/auth/authReturn'
import { THIRDWEB_GUEST_PARAM } from '@explorer-core/features/auth/guestWalletParam'
import type { AcceptedParam } from '@explorer-core/hostContract'
import { WEB_PARAMS } from '@explorer-core/engine/generated'
import { DEBUG_PANEL_PARAM } from '@explorer-core/lib/debugPanelLauncher'
import { MOCK_SWITCHES } from '@explorer-core/engine/mockSwitches'
import { ANALYTICS_SESSION_PARAM, isAttributionParam } from '@explorer-core/lib/analytics'

export type { AcceptedParam } from '@explorer-core/hostContract'

// The page's own user-facing params — read by App / bootMode, never handed to the engine.
const PAGE_PARAMS: Record<string, string> = {
  world: 'A world by name: `world=boedo` is `realm=boedo.dcl.eth`.',
  guest: '`1` skips the sign-in screen with a guest login.',
  fps: '`1` shows the frame-rate overlay (Ctrl/Cmd+Shift+F toggles it).',
  pulseCertificateHash:
    'SHA-256 (hex or base64) of a self-signed Pulse server certificate to trust, for a local or test server named by `pulseServer=` (e.g. `pulseServer=127.0.0.1:7743`).'
}

const WORLD_NAME = /^[a-z0-9-]+$/

/** The realm a link names: `realm`, else `world`, a name under `.dcl.eth` given bare or in full.
 *  Anything else as a world (a url, another ens) is no destination. */
export function entryRealm(q: URLSearchParams): string | null {
  const realm = q.get('realm')
  if (realm != null && realm !== '') return realm
  const world = q.get('world')?.trim().toLowerCase().replace(/\.dcl\.eth$/, '')
  return world != null && WORLD_NAME.test(world) ? `${world}.dcl.eth` : null
}

// Also read by this page, but not a user's to set: forced by the host (native, hud), or debug /
// test switches (mock, showcase, the gate overrides, the mock-bridge fixtures). Accepted without a
// warning, not advertised.
const INTERNAL_PARAMS = [
  'native',
  DEBUG_PANEL_PARAM,
  'hud',
  'mock',
  'showcase',
  // `shop=old`: the catalogue page the in-world Shop replaced, while it is retired (`shop=new`, the old
  // switch, is still accepted)
  'shop',
  'gate',
  'nogate',
  // forces the integrated-GPU guide (features/gate/IntegratedGpuGuide.tsx) for a look at it
  'gpuGuide',
  'bundled',
  'simerror',
  'simhover',
  // the mock bridge's own switches, the same in both hosts
  ...MOCK_SWITCHES,
  'perm',
  'worldpw',
  ANALYTICS_SESSION_PARAM,
  AUTH_RETURN_PARAM,
  // the shell's choice of guest wallet (features/auth/guest.ts)
  THIRDWEB_GUEST_PARAM
]

/** What the dialog advertises: every link-settable engine param, then the page's own. Not
 *  `autopilot`, the benchmark's run, which /play never carries: a link with it is told it was ignored. */
export function acceptedEntryParams(): AcceptedParam[] {
  const engine = WEB_PARAMS.filter((p) => p.delivery !== 'host' && p.name !== 'autopilot').map((p) => ({
    name: p.name,
    doc: p.doc,
    reader: 'engine' as const
  }))
  const page = Object.entries(PAGE_PARAMS).map(([name, doc]) => ({ name, doc, reader: 'page' as const }))
  return [...engine, ...page]
}

/** The entry url's params that nothing reads — ignored, and worth telling the user about.
 *  `hostParams`: host-delivered params this page is acting on. */
export function unrecognisedEntryParams(q: URLSearchParams, hostParams: string[] = []): string[] {
  const accepted = new Set([...acceptedEntryParams().map((p) => p.name), ...INTERNAL_PARAMS, ...hostParams])
  return [...new Set(q.keys())].filter((name) => !accepted.has(name) && !isAttributionParam(name))
}
