import { describe, expect, it } from 'vitest'
import { judgeWorldAccess } from '../features/places/worldAccess'

const OWNER = '0xOwner00000000000000000000000000000000001'
const ME = '0xMe0000000000000000000000000000000000002'

describe('world access', () => {
  it('lets anyone into an unrestricted world, or one with no access setting', () => {
    expect(judgeWorldAccess({ owner: OWNER, permissions: { access: { type: 'unrestricted' } } }, ME, [])).toBe('allowed')
    expect(judgeWorldAccess({ owner: OWNER, permissions: {} }, null, [])).toBe('allowed')
  })

  it('asks for the password unless the wallet owns the world', () => {
    expect(judgeWorldAccess({ owner: OWNER, permissions: { access: { type: 'shared-secret' } } }, ME, [])).toBe('password')
    expect(judgeWorldAccess({ owner: OWNER, permissions: { access: { type: 'shared-secret' } } }, OWNER.toLowerCase(), [])).toBe('allowed')
  })

  it('invites a listed wallet, the owner or a member of a listed community, and denies the rest', () => {
    const list = (wallets: string[], communities: string[] = []) => ({ owner: OWNER, permissions: { access: { type: 'allow-list', wallets, communities } } })
    expect(judgeWorldAccess(list([ME.toUpperCase()]), ME, [])).toBe('invited')
    expect(judgeWorldAccess(list([]), OWNER, [])).toBe('invited')
    expect(judgeWorldAccess(list([], ['c1']), ME, ['c1'])).toBe('invited')
    expect(judgeWorldAccess(list(['0xsomeoneelse']), ME, ['c2'])).toBe('denied')
    expect(judgeWorldAccess(list([ME]), null, [])).toBe('denied')
  })

  it('leaves a world only a community opens undecided until the communities are known', () => {
    const list = { owner: OWNER, permissions: { access: { type: 'allow-list', wallets: [], communities: ['c1'] } } }
    expect(judgeWorldAccess(list, ME, null)).toBe('failed')
    expect(judgeWorldAccess(list, ME, [])).toBe('denied')
    expect(judgeWorldAccess(list, OWNER, null)).toBe('invited')
    expect(judgeWorldAccess({ owner: OWNER, permissions: { access: { type: 'allow-list', wallets: [] } } }, ME, null)).toBe('denied')
  })

  it('cannot tell for an access type it does not know', () => {
    expect(judgeWorldAccess({ owner: OWNER, permissions: { access: { type: 'nft-gated' } } }, ME, [])).toBe('failed')
  })
})
