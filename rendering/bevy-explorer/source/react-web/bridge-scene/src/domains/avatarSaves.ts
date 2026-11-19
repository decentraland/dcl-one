import { getPlayer } from '@dcl/sdk/players'

type AccountSaves = {
  address: string | null
  pending: Set<Promise<unknown>>
  tail?: Promise<unknown>
}

let active: AccountSaves | undefined

export function observeAvatarSaveAccount(address: string | null): void {
  if (active?.address !== address) active = undefined
}

export function avatarSaveSession(address: string | null) {
  if (active?.address !== address) active = { address, pending: new Set() }
  const account = active
  const isCurrent = (): boolean => active === account && (getPlayer()?.userId ?? null) === address

  const run = <T>(write: () => Promise<T>, kind: 'passport' | 'avatar'): Promise<T | undefined> => {
    // setAvatar may replace a pending request, and the engine only takes the last request
    // in a frame. Give each Passport save its own deploy result. Avatar writes queued behind
    // it also drain in order so opening the gate cannot submit them all in one frame.
    const wait = kind === 'passport' ? Promise.allSettled(account.pending) : account.tail?.catch(() => undefined)
    const submit = async (): Promise<T | undefined> => isCurrent() ? await write() : undefined
    const result = wait == null ? submit() : wait.then(submit)
    account.pending.add(result)
    if (wait != null) account.tail = result
    const release = (): void => {
      account.pending.delete(result)
      if (account.tail === result) account.tail = undefined
    }
    void result.then(release, release)
    return result
  }

  return { run, isCurrent }
}
