import { getPlayer } from '@dcl/sdk/players'
import { waitMs } from '../system-helpers'

// Leave room for two 60-second deployment attempts and a Retry-After of up to 60 seconds.
const QUEUE_WAIT_MS = 4 * 60_000

type AccountSaves = {
  address: string | null
  pending: Set<Promise<unknown>>
  tail?: Promise<unknown>
}

let active: AccountSaves | undefined

export function resetAvatarSaveAccount(): void {
  active = undefined
}

export function observeAvatarSaveAccount(address: string | null): void {
  // Welcome can start saving the stand-in before the engine publishes the player.
  if (active?.address === null && address !== null) active.address = address
  else if (active?.address !== address) resetAvatarSaveAccount()
}

export function avatarSaveSession(address: string | null) {
  observeAvatarSaveAccount(address)
  const account = active ??= { address, pending: new Set() }
  const isCurrent = (): boolean => {
    if (active !== account) return false
    const player = getPlayer()?.userId ?? null
    observeAvatarSaveAccount(player)
    return active === account
  }

  const run = <T>(write: () => Promise<T>, kind: 'passport' | 'avatar'): Promise<T | undefined> => {
    // Passport waits for active writes, then owns its deployment until the engine answers.
    // Avatar-only writes can overlap. The ones queued behind Passport drain in order.
    const wait = kind === 'passport' && account.pending.size > 0
      ? Promise.allSettled(account.pending)
      : account.tail?.catch(() => undefined)
    let waiting = wait != null
    let expired = false
    const submit = async (): Promise<T | undefined> => {
      waiting = false
      if (expired || !isCurrent()) return undefined
      return await write()
    }
    const result = wait == null ? submit() : wait.then(submit)
    account.pending.add(result)
    if (kind === 'passport' || wait != null) account.tail = result
    const release = (): void => {
      account.pending.delete(result)
      if (account.tail === result) account.tail = undefined
    }
    void result.then(release, release)
    if (wait == null) return result

    // Only an undispatched save may expire. Keep the internal chain behind the live write
    // until the engine settles it, even if the caller has already been asked to retry.
    const deadline = waitMs(QUEUE_WAIT_MS).then(() => {
      if (!waiting) return result
      expired = true
      if (!isCurrent()) return undefined
      throw new Error('A previous avatar save is taking too long. Try again.')
    })
    return Promise.race([result, deadline])
  }

  return { run, isCurrent }
}

export type AvatarSaveSession = ReturnType<typeof avatarSaveSession>
