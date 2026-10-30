import { getPlayer } from '@dcl/sdk/players'
import { BevyApi } from './bevy-api'
import { waitFor } from './utils'

async function playerWithin(ms: number): Promise<boolean> {
  try {
    await waitFor(() => getPlayer() !== null, ms)
    return true
  } catch {
    return false
  }
}

export async function autoLogin(): Promise<void> {
  if (getPlayer() !== null) return
  const previous = await BevyApi.getPreviousLogin().catch(() => null)
  if (getPlayer() !== null) return

  if (previous?.userId !== null && previous?.userId !== undefined) {
    console.log('found previous login', previous.userId, '- logging in')
    const result = await BevyApi.loginPrevious()
      .then(() => ({ success: true, error: '' }))
      .catch((e: unknown) => ({ success: false, error: String(e) }))
    if (result.success && (await playerWithin(10000))) {
      console.log('logged in (previous), player present')
      return
    }
    console.error('previous login failed, falling back to guest:', result.error || 'no player appeared')
  } else {
    console.log('no previous login - logging in as guest')
  }

  for (let attempt = 0; attempt < 5; attempt++) {
    try {
      BevyApi.loginGuest()
    } catch (e) {
      console.error('loginGuest threw:', e)
    }
    if (await playerWithin(8000)) {
      console.log('logged in as guest, player present')
      return
    }
    console.log(`guest login attempt ${attempt + 1}: no player yet, retrying…`)
  }

  console.error('autoLogin: player never appeared after retries — continuing anyway')
}
