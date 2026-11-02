// What a run targets, from the environment. One setting set picks the backend (zone by default),
// the client build under test and the accounts; a run never logs a key.
import { DEFAULT_WEB_APP } from './paths.ts'

export type Transport = 'web' | 'native'

export interface Target {
  /** The backend's base domain: the HUD's services derive from it. */
  baseDomain: string
  /** The realm the client boots into: a realm URL or a world name. */
  realm: string
  /** The spawn parcel, `x,y`. */
  position: string
  transport: Transport
  /** The web app folder to serve on loopback (the CI artifact, or `make web`'s `dist/web`). */
  webApp: string
  /** An already served web app to open instead of serving `webApp` (zone's `/play` for a smoke). */
  webUrl?: string
  chrome?: string
  headed: boolean
  windowSize: [number, number]
  /** Account private keys, one per worker; never logged. */
  accounts: string[]
  worker: number
  ports: { web: number; realm: number }
  /** Record missing visual references instead of failing on them. */
  record: boolean
  native: { bin?: string; port?: number; token?: string; args: string[] }
}

export const DEFAULT_BASE_DOMAIN = 'decentraland.zone'
export const DEFAULT_REALM = 'https://peer.decentraland.zone'
export const DEFAULT_POSITION = '100,100'
export const DEFAULT_WEB_PORT = 18743
export const DEFAULT_REALM_PORT = 18744

function flag(value: string | undefined): boolean {
  return value === '1' || value === 'true'
}

function integer(value: string | undefined, fallback: number, name: string): number {
  if (value === undefined || value === '') return fallback
  const parsed = Number(value)
  if (!Number.isInteger(parsed) || parsed < 0) throw new Error(`${name} must be a non-negative integer, not "${value}"`)
  return parsed
}

export function parseWindowSize(value: string | undefined, fallback: [number, number] = [1600, 900]): [number, number] {
  if (value === undefined || value === '') return fallback
  const match = /^(\d+)x(\d+)$/.exec(value)
  if (!match) throw new Error(`DCL_E2E_WINDOW must be <width>x<height>, not "${value}"`)
  return [Number(match[1]), Number(match[2])]
}

export function readTarget(env: NodeJS.ProcessEnv = process.env): Target {
  const transport = env.DCL_E2E_TRANSPORT === 'native' ? 'native' : 'web'
  if (env.DCL_E2E_TRANSPORT && env.DCL_E2E_TRANSPORT !== transport) {
    throw new Error(`DCL_E2E_TRANSPORT must be web or native, not "${env.DCL_E2E_TRANSPORT}"`)
  }
  const accounts = (env.DCL_E2E_ACCOUNT_KEYS ?? '')
    .split(',')
    .map((key) => key.trim())
    .filter((key) => key !== '')
  for (const key of accounts) {
    if (!/^0x[0-9a-fA-F]{64}$/.test(key)) throw new Error('DCL_E2E_ACCOUNT_KEYS holds something that is not a 0x-prefixed 32-byte hex key')
  }
  return {
    baseDomain: env.DCL_E2E_BASE_DOMAIN || DEFAULT_BASE_DOMAIN,
    realm: env.DCL_E2E_REALM || DEFAULT_REALM,
    position: env.DCL_E2E_POSITION || DEFAULT_POSITION,
    transport,
    webApp: env.DCL_E2E_WEB_APP || DEFAULT_WEB_APP,
    webUrl: env.DCL_E2E_WEB_URL || undefined,
    chrome: env.CHROME_PATH || undefined,
    headed: !flag(env.DCL_E2E_HEADLESS),
    windowSize: parseWindowSize(env.DCL_E2E_WINDOW),
    accounts,
    worker: integer(env.DCL_E2E_WORKER, 0, 'DCL_E2E_WORKER'),
    ports: {
      web: integer(env.DCL_E2E_WEB_PORT, DEFAULT_WEB_PORT, 'DCL_E2E_WEB_PORT'),
      realm: integer(env.DCL_E2E_REALM_PORT, DEFAULT_REALM_PORT, 'DCL_E2E_REALM_PORT')
    },
    record: flag(env.DCL_E2E_RECORD),
    native: {
      bin: env.DCL_E2E_NATIVE_BIN || undefined,
      port: env.DCL_E2E_NATIVE_PORT ? integer(env.DCL_E2E_NATIVE_PORT, 0, 'DCL_E2E_NATIVE_PORT') : undefined,
      token: env.DCL_E2E_NATIVE_TOKEN || undefined,
      args: (env.DCL_E2E_NATIVE_ARGS ?? '').split(/\s+/).filter((arg) => arg !== '')
    }
  }
}

/** This worker's account key. */
export function accountKey(target: Target): string {
  const key = target.accounts[target.worker % Math.max(target.accounts.length, 1)]
  if (!key) throw new Error('no account: set DCL_E2E_ACCOUNT_KEYS (comma-separated 0x private keys, one per worker)')
  return key
}

/** The target for a log, with nothing secret in it. */
export function describeTarget(target: Target): string {
  return [
    `transport=${target.transport}`,
    `baseDomain=${target.baseDomain}`,
    `realm=${target.realm}`,
    `position=${target.position}`,
    target.webUrl ? `webUrl=${target.webUrl}` : `webApp=${target.webApp}`,
    `accounts=${target.accounts.length}`,
    `worker=${target.worker}`,
    `headed=${target.headed}`,
    `window=${target.windowSize[0]}x${target.windowSize[1]}`,
    `record=${target.record}`
  ].join(' ')
}
