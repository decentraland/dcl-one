// The creators flags, read by the server. Decentraland's flag service answers by the site a request comes
// from (its Referer), so a browser on any other domain would be told every flag is off: the server asks as
// the environment's Decentraland site and the page reads the answer from this app.
import { ttlMemo } from '../ttl-memo'
import { withDeadline } from '../request-deadline'
import type { EnvConfig } from './config/env'

const TIMEOUT_MS = 1_500
const TTL_MS = 60_000

export const FLAG_APPLICATIONS = ['builder', 'dapps'] as const
export type FlagApplication = (typeof FLAG_APPLICATIONS)[number]

type Flags = Record<string, boolean>

async function load(service: string, site: string, application: FlagApplication): Promise<Flags> {
  return withDeadline(async signal => {
    const response = await fetch(`${service}/${application}.json`, { headers: { Referer: `${site}/` }, signal })
    if (!response.ok) {
      await response.body?.cancel().catch(() => undefined)
      throw new Error(`feature flags answered ${response.status}`)
    }
    const body = (await response.json()) as { flags?: Record<string, unknown> | null }
    const flags: Flags = {}
    for (const [key, value] of Object.entries(body.flags ?? {})) flags[key] = value === true
    return flags
  }, TIMEOUT_MS)
}

// One read a minute for each service, site and application; a failed read is not kept, so the next one retries.
const read = ttlMemo({
  ttlMs: TTL_MS,
  keyOf: (service: string, site: string, application: FlagApplication) => `${service}|${site}|${application}`,
  load
})

/** An application's flags in the environment `config` names, each on only where the service says `true`. Rejects when they cannot be read. */
export function readFlags(config: EnvConfig, application: FlagApplication): Promise<Flags> {
  return read(config.get('FEATURE_FLAGS_URL'), config.get('SITES_URL'), application)
}

export function resetFlags(): void {
  read.reset()
}
