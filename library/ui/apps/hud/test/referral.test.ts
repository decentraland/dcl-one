// The referral notifications (notifications/summary's referralTarget) and the invite link the Me panel
// copies (sidebar/inviteLink): who gets a link, and where each referral notification leads.
import { afterEach, describe, expect, it, vi } from 'vitest'

const passport = vi.hoisted(() => vi.fn())
vi.mock('../features/profile/Passport', async (importOriginal) => ({ ...(await importOriginal<object>()), openPassport: passport }))

import { siteUrl } from '@hud-host'
import { referralTarget } from '../features/notifications/summary'

const REFERRER = '0x' + 'AB'.repeat(20)
const INVITED = '0x' + 'cd'.repeat(20)
const notice = (type: string, metadata: Record<string, unknown>) =>
  ({ id: '1', type, address: REFERRER, timestamp: '0', read: false, metadata }) as never

describe('referral notifications', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    passport.mockReset()
  })

  it('an accepted invite opens the invited player’s passport', () => {
    referralTarget(notice('referral_invited_users_accepted', { address: REFERRER, invitedUserAddress: INVITED }))?.()
    expect(passport).toHaveBeenCalledWith(INVITED)
  })

  it('a new tier opens the referrer’s Referral Rewards page in a new tab', () => {
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    referralTarget(notice('referral_new_tier_reached', { address: REFERRER, tier: 2 }))?.()
    expect(open).toHaveBeenCalledWith(siteUrl(`/profile/${REFERRER.toLowerCase()}/referral-rewards`), '_blank', 'noopener')
    expect(passport).not.toHaveBeenCalled()
  })

  it('leads nowhere without the address it needs, or for another type', () => {
    expect(referralTarget(notice('referral_invited_users_accepted', { address: REFERRER }))).toBeUndefined()
    expect(referralTarget(notice('referral_new_tier_reached', {}))).toBeUndefined()
    expect(referralTarget(notice('tip_received', { invitedUserAddress: INVITED }))).toBeUndefined()
  })
})

describe('the invite link', () => {
  it('is the site’s invite page for a signed-in player’s wallet', async () => {
    const { inviteLink } = await import('../features/sidebar/inviteLink')
    expect(inviteLink({ address: INVITED, isGuest: false })).toBe(siteUrl(`/invite/${INVITED}`))
  })

  it('is none for a guest, a player whose kind is unknown, or no wallet address', async () => {
    const { inviteLink } = await import('../features/sidebar/inviteLink')
    expect(inviteLink({ address: INVITED, isGuest: true })).toBeUndefined()
    expect(inviteLink({ address: INVITED } as never)).toBeUndefined()
    expect(inviteLink({ address: 'guest', isGuest: false })).toBeUndefined()
    expect(inviteLink(null)).toBeUndefined()
  })
})
