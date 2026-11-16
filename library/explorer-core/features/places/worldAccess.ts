import { serviceUrl } from '@hud-host'

/** `invited`: let in by an allow list. `failed`: the check could not tell, so Jump in stays and
 *  the world decides on arrival. */
export type WorldAccess = 'allowed' | 'invited' | 'denied' | 'password' | 'failed'

interface PermissionsBody {
  owner?: string
  permissions?: { access?: { type?: string; wallets?: string[]; communities?: string[] } | null }
}

const same = (a: string | null | undefined, b: string | null | undefined): boolean => a != null && b != null && a !== '' && a.toLowerCase() === b.toLowerCase()

/** `communities` is null while unknown: a world only a community could open is then `failed`. */
export function judgeWorldAccess(body: PermissionsBody, wallet: string | null, communities: readonly string[] | null): WorldAccess {
  const access = body.permissions?.access
  const type = access?.type?.toLowerCase() ?? ''
  if (type === '' || type === 'unrestricted') return 'allowed'
  if (type === 'shared-secret') return same(wallet, body.owner) ? 'allowed' : 'password'
  if (type !== 'allow-list') return 'failed'
  if ((access?.wallets ?? []).some((w) => same(w, wallet)) || same(wallet, body.owner)) return 'invited'
  const listed = access?.communities ?? []
  if (listed.length > 0 && communities == null) return 'failed'
  return listed.some((id) => communities?.includes(id) === true) ? 'invited' : 'denied'
}

export async function fetchWorldAccess(world: string, wallet: string | null, communities: readonly string[] | null, signal?: AbortSignal): Promise<WorldAccess> {
  try {
    const res = await fetch(`${serviceUrl('worldsServer')}/world/${encodeURIComponent(world)}/permissions`, { signal })
    if (!res.ok) {
      await res.body?.cancel().catch(() => undefined)
      return 'failed'
    }
    return judgeWorldAccess((await res.json()) as PermissionsBody, wallet, communities)
  } catch (e) {
    if (signal?.aborted) throw e
    return 'failed'
  }
}
