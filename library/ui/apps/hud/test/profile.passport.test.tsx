import { afterEach, describe, it, expect, vi } from 'vitest'
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { ProfilePassport, type PassportAvatarSlot } from '../features/profile/ProfilePassport'
import type { Emote, GalleryPhoto, Profile, Wearable } from '@explorer-core/engine/protocol'
import { setPublicReelFetcher } from '../features/gallery/publicReels'
import type { PassportPhotoActions } from '../features/gallery/PassportPhotos'
import { PopupHost } from '../design'

afterEach(() => setPublicReelFetcher(null))

const profile: Profile = {
  address: '0xkurd000000000000000000000000000000006b635',
  name: 'kurd',
  picture: 'k.png',
  hasClaimedName: true,
  isGuest: false,
  description: 'old gamer in dcl',
  links: [{ title: 'x account', url: 'https://x.com/kurd' }],
  badges: [
    { id: 'festive', name: 'Festive Trail', description: 'Followed the trail', image: 'https://x/festive.png', unlockedAt: Date.UTC(2024, 11, 4, 14, 16, 34) },
    {
      id: 'traveler',
      name: 'Traveler',
      description: 'Scenes explored',
      tier: 'Starter',
      unlockedAt: Date.UTC(2024, 10, 15, 12),
      progress: { done: 5, target: 50 },
      tiers: [
        { id: 'traveler-starter', name: 'Starter', description: '1 scene explored', unlockedAt: Date.UTC(2024, 10, 15, 12) },
        { id: 'traveler-bronze', name: 'Bronze', description: '50 scenes explored' }
      ]
    },
    { id: 'voyager', name: 'Vertical Voyager', description: 'Reached 500m', image: 'https://x/voyager.png', locked: true }
  ],
  info: { gender: 'Male', realName: 'mohammad', language: 'Persian' }
}

describe('profile passport', () => {
  it('renders the overview: name, about, fields, links', () => {
    render(<ProfilePassport profile={profile} onClose={vi.fn()} />)
    expect(screen.getByText('kurd')).toBeInTheDocument()
    expect(screen.getByText('old gamer in dcl')).toBeInTheDocument()
    expect(screen.getByText('mohammad')).toBeInTheDocument() // Real Name field value
    expect(screen.getByRole('link', { name: /x account/i })).toHaveAttribute('href', 'https://x.com/kurd')
  })

  it('shows up to three mutual friends and how many there are, on another user only', () => {
    const mutuals = ['Ana', 'Bo', 'Cy', 'Di'].map((name, i) => ({ address: `0x${i}`, name }))
    const { rerender } = render(<ProfilePassport profile={profile} mutuals={mutuals} onClose={vi.fn()} />)
    const row = screen.getByText('4 Mutual').parentElement as HTMLElement
    expect(Array.from(row.children).map((el) => el.textContent)).toEqual(['A', 'B', 'C', '4 Mutual'])
    rerender(<ProfilePassport profile={profile} mutuals={[]} onClose={vi.fn()} />)
    expect(screen.queryByText(/Mutual/)).toBeNull()
    rerender(<ProfilePassport profile={profile} mutuals={mutuals} isSelf onClose={vi.fn()} />)
    expect(screen.queryByText(/Mutual/)).toBeNull()
  })

  it('shortens the wallet to five and five characters and says what each copy button copied', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    vi.stubGlobal('navigator', { ...navigator, clipboard: { writeText } })
    render(<ProfilePassport profile={profile} onClose={vi.fn()} />)
    expect(screen.getByText('0xkur...6b635')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Copy name' }))
    expect(writeText).toHaveBeenLastCalledWith('kurd')
    expect((await screen.findByText('Name copied!')).parentElement).toContainElement(screen.getByRole('button', { name: 'Copy name' }))
    fireEvent.click(screen.getByRole('button', { name: 'Copy address' }))
    expect(writeText).toHaveBeenLastCalledWith(profile.address)
    expect(await screen.findByText('User ID copied!')).toBeInTheDocument()
    vi.unstubAllGlobals()
  })

  it('shows the #tag of an unclaimed name only, and copies the name with it', () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    vi.stubGlobal('navigator', { ...navigator, clipboard: { writeText } })
    const { rerender } = render(<ProfilePassport profile={{ ...profile, name: 'kurd#b635', hasClaimedName: false }} onClose={vi.fn()} />)
    expect(screen.getByText('#b635')).toBeInTheDocument()
    expect(screen.queryByLabelText('verified')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Copy name' }))
    expect(writeText).toHaveBeenLastCalledWith('kurd#b635')
    rerender(<ProfilePassport profile={profile} onClose={vi.fn()} />)
    expect(screen.queryByText(/^#/)).toBeNull()
    expect(screen.getByLabelText('verified')).toBeInTheDocument()
    vi.unstubAllGlobals()
  })

  it('marks an official wallet', () => {
    const { rerender } = render(<ProfilePassport profile={profile} onClose={vi.fn()} />)
    expect(screen.queryByText('DCL Foundation')).toBeNull()
    rerender(<ProfilePassport profile={{ ...profile, official: true }} onClose={vi.fn()} />)
    expect(screen.getByText('DCL Foundation')).toBeInTheDocument()
  })

  it('orders the header actions: friendship, Chat, Call, Jump to Location, the menu, Close', () => {
    render(<ProfilePassport profile={profile} relationship="friend" onMessage={vi.fn()} onCall={vi.fn()} onJump={vi.fn()} onReport={vi.fn()} onClose={vi.fn()} />)
    const actions = screen.getByRole('button', { name: 'Close' }).parentElement as HTMLElement
    expect(within(actions).getAllByRole('button').map((b) => b.getAttribute('aria-label') ?? b.textContent)).toEqual([
      'FRIENDREMOVE FRIEND',
      'Chat',
      'Call',
      'Jump to Location',
      'More options',
      'Close'
    ])
  })

  it('starts the options menu with the user, when it has anything to offer', async () => {
    const { rerender } = render(<ProfilePassport profile={profile} onReport={vi.fn()} onClose={vi.fn()} />)
    await userEvent.click(screen.getByRole('button', { name: 'More options' }))
    const menu = screen.getByRole('menu')
    expect(within(menu).getByText('kurd')).toBeInTheDocument()
    expect(menu.firstElementChild).toContainElement(within(menu).getByText('kurd'))
    rerender(<ProfilePassport profile={profile} onClose={vi.fn()} />)
    expect(screen.queryByRole('button', { name: 'More options' })).toBeNull()
  })

  it("draws the user's picture with no online dot, as unity-explorer's passport does", () => {
    render(<ProfilePassport profile={profile} onClose={vi.fn()} />)
    const picture = screen.getAllByRole('img', { name: 'kurd' })[0]
    expect(picture.parentElement?.querySelector('[class*="hudDot"]')).toBeNull()
  })

  describe('when its host gives it the live avatar', () => {
    const slot = vi.fn((given: PassportAvatarSlot) => <div data-testid="live-avatar" data-background={given.background} data-wave={String(given.wave)} />)
    afterEach(() => slot.mockClear())

    it("should show it in the column in the picture's place, over the background of the tab on show", async () => {
      render(<ProfilePassport profile={{ ...profile, bodyImage: 'https://x/body.png' }} avatar={slot} onClose={vi.fn()} />)
      expect(screen.getByTestId('live-avatar')).toHaveAttribute('data-background', 'overview')
      expect(screen.queryByRole('img', { name: 'kurd' })).toBeNull()
      await userEvent.click(screen.getByRole('tab', { name: 'PHOTOS' }))
      expect(screen.getByTestId('live-avatar')).toHaveAttribute('data-background', 'photos')
    })

    it('should hand it the picture to show when it cannot be drawn', () => {
      render(<ProfilePassport profile={{ ...profile, bodyImage: 'https://x/body.png' }} avatar={slot} onClose={vi.fn()} />)
      const { container } = render(<>{slot.mock.calls[0][0].picture}</>)
      expect(container.querySelector('img')).toHaveAttribute('src', 'https://x/body.png')
    })

    it('should hide it on the Badges tab, and not have it wave when it comes back', async () => {
      render(<ProfilePassport profile={{ ...profile, badges: [] }} avatar={slot} onClose={vi.fn()} />)
      expect(screen.getByTestId('live-avatar')).toHaveAttribute('data-wave', 'true')
      await userEvent.click(screen.getByRole('tab', { name: 'BADGES' }))
      expect(screen.queryByTestId('live-avatar')).toBeNull()
      await userEvent.click(screen.getByRole('tab', { name: 'OVERVIEW' }))
      expect(screen.getByTestId('live-avatar')).toHaveAttribute('data-wave', 'false')
    })

    it('should open the panel background over the column only while the avatar says it is drawn', () => {
      const { container } = render(<ProfilePassport profile={profile} avatar={slot} onClose={vi.fn()} />)
      const panel = container.querySelector('[data-passport-panel]')
      expect(panel).not.toHaveAttribute('data-live-avatar')
      act(() => slot.mock.calls[0][0].onLive(true))
      expect(panel).toHaveAttribute('data-live-avatar')
      act(() => slot.mock.calls[0][0].onLive(false))
      expect(panel).not.toHaveAttribute('data-live-avatar')
    })

    it('should wait with a spinner while the avatar is not known yet', () => {
      const { container } = render(<ProfilePassport profile={profile} avatarPending onClose={vi.fn()} />)
      expect(within(container.querySelector<HTMLElement>('[data-passport-avatar]')!).getByRole('status')).toBeInTheDocument()
      expect(screen.queryByRole('img', { name: 'kurd' })).toBeNull()
    })
  })

  it.each([
    ['none', 'ADD FRIEND', 'onAddFriend'],
    ['friend', 'FRIEND', 'onUnfriend'],
    ['requested', 'REQUEST SENT', 'onCancelRequest'],
    ['incoming', 'ACCEPT FRIEND', 'onAcceptRequest'],
    ['blocked', 'BLOCKED', 'onUnblock']
  ] as const)('the friendship button for %s reads %s and calls %s', async (relationship, label, handler) => {
    const fn = vi.fn()
    render(<ProfilePassport profile={profile} relationship={relationship} {...{ [handler]: fn }} onClose={vi.fn()} />)
    await userEvent.click(screen.getByRole('button', { name: new RegExp(`^${label}`) }))
    expect(fn).toHaveBeenCalledWith(profile.address)
  })

  it('offers Block and Report in the options menu, and calls each with the address', async () => {
    const onBlock = vi.fn()
    const onReport = vi.fn()
    render(<ProfilePassport profile={profile} onBlock={onBlock} onReport={onReport} onClose={vi.fn()} />)
    expect(screen.queryByRole('menu')).toBeNull()
    // A menu button, not a toggle: aria-expanded and aria-haspopup, never aria-pressed.
    expect(screen.getByRole('button', { name: 'More options' })).not.toHaveAttribute('aria-pressed')
    await userEvent.click(screen.getByRole('button', { name: 'More options' }))
    await userEvent.click(screen.getByRole('menuitem', { name: 'Block' }))
    expect(onBlock).toHaveBeenCalledWith(profile.address)
    expect(screen.queryByRole('menu')).toBeNull() // picking an item closes the menu
    await userEvent.click(screen.getByRole('button', { name: 'More options' }))
    await userEvent.click(screen.getByRole('menuitem', { name: 'Report' }))
    expect(onReport).toHaveBeenCalledWith(profile.address)
  })

  it('leaves Block out of the menu for a user you already blocked', async () => {
    render(<ProfilePassport profile={profile} relationship="blocked" onBlock={vi.fn()} onReport={vi.fn()} onClose={vi.fn()} />)
    await userEvent.click(screen.getByRole('button', { name: 'More options' }))
    expect(screen.queryByRole('menuitem', { name: 'Block' })).toBeNull()
    expect(screen.getByRole('menuitem', { name: 'Report' })).toBeInTheDocument()
  })

  it('has no options menu on your own passport', () => {
    render(<ProfilePassport profile={profile} isSelf onBlock={vi.fn()} onReport={vi.fn()} onClose={vi.fn()} />)
    expect(screen.queryByRole('button', { name: 'More options' })).toBeNull()
  })

  it.each([
    ['requested', 'REQUEST SENT', 'CANCEL REQUEST'],
    ['friend', 'FRIEND', 'REMOVE FRIEND'],
    ['blocked', 'BLOCKED', 'UNBLOCK']
  ] as const)('the friendship button for %s reads %s and offers %s on hover', (relationship, idle, hover) => {
    render(<ProfilePassport profile={profile} relationship={relationship} onClose={vi.fn()} />)
    const button = screen.getByRole('button', { name: new RegExp(`^${idle}`) })
    expect(within(button).getByText(idle)).toBeInTheDocument()
    expect(within(button).getByText(hover)).toBeInTheDocument()
  })

  it('offers no Chat or Call for a user you blocked', () => {
    render(<ProfilePassport profile={profile} relationship="blocked" onMessage={vi.fn()} onCall={vi.fn()} onUnblock={vi.fn()} onClose={vi.fn()} />)
    expect(screen.queryByRole('button', { name: 'Chat' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Call' })).toBeNull()
  })

  it('offers no Chat, Call or friendship button when the user blocked you, but keeps Block and Report', async () => {
    render(<ProfilePassport profile={profile} blockedBy onMessage={vi.fn()} onCall={vi.fn()} onAddFriend={vi.fn()} onBlock={vi.fn()} onReport={vi.fn()} onClose={vi.fn()} />)
    expect(screen.queryByRole('button', { name: 'Chat' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Call' })).toBeNull()
    expect(screen.queryByRole('button', { name: /FRIEND/ })).toBeNull()
    await userEvent.click(screen.getByRole('button', { name: 'More options' }))
    expect(screen.getByRole('menuitem', { name: 'Block' })).toBeInTheDocument()
    expect(screen.getByRole('menuitem', { name: 'Report' })).toBeInTheDocument()
  })

  it('still offers UNBLOCK when you blocked a user who also blocked you', () => {
    render(<ProfilePassport profile={profile} relationship="blocked" blockedBy onUnblock={vi.fn()} onClose={vi.fn()} />)
    expect(screen.getByRole('button', { name: /UNBLOCK$/ })).toBeInTheDocument()
  })

  it('hides the friend action on your own passport (isSelf)', () => {
    render(<ProfilePassport profile={profile} isSelf onClose={vi.fn()} />)
    expect(screen.queryByRole('button', { name: /FRIEND/i })).toBeNull()
  })

  it('uses the full-body snapshot as the avatar when present', () => {
    render(<ProfilePassport profile={{ ...profile, bodyImage: 'https://x/body.png' }} onClose={vi.fn()} />)
    expect(screen.getAllByRole('img').some((i) => i.getAttribute('src') === 'https://x/body.png')).toBe(true)
  })

  it('close button closes', async () => {
    const onClose = vi.fn()
    render(<ProfilePassport profile={profile} onClose={onClose} />)
    await userEvent.click(screen.getByRole('button', { name: 'Close' }))
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('the Overview row shows unlocked badges only', () => {
    render(<ProfilePassport profile={profile} onClose={vi.fn()} />)
    expect(screen.getByAltText('Festive Trail')).toBeInTheDocument()
    expect(screen.queryByAltText('Vertical Voyager')).not.toBeInTheDocument()
  })

  describe('Badges tab', () => {
    const open = async (isSelf = false) => {
      render(<ProfilePassport profile={profile} isSelf={isSelf} onClose={vi.fn()} />)
      await userEvent.click(screen.getByRole('tab', { name: 'BADGES' }))
    }

    it('lists every badge as a card with its unlock month, a dash while locked', async () => {
      await open()
      const cards = screen.getAllByRole('button').filter((b) => b.hasAttribute('aria-pressed'))
      expect(cards.map((c) => c.textContent)).toEqual(['Festive TrailDec. 2024', 'TravelerNov. 2024', 'Vertical Voyager—'])
      expect(screen.getByText('Dec. 2024')).toHaveAttribute('dateTime', '2024-12-04T14:16:34.000Z')
    })

    it('opens on the first badge and shows the picked one in place of the avatar', async () => {
      await open()
      expect(screen.getByRole('heading', { name: 'Festive Trail' })).toBeInTheDocument()
      expect(screen.getByText('Unlocked: Dec. 2024')).toBeInTheDocument()
      expect(screen.getByText('Followed the trail')).toBeInTheDocument()
      await userEvent.click(screen.getByRole('button', { name: /^Vertical Voyager/ }))
      expect(screen.getByRole('heading', { name: 'Vertical Voyager' })).toBeInTheDocument()
      expect(screen.getByText('Locked')).toBeInTheDocument()
    })

    it("a tiered badge opens on its latest tier; someone else's passport lists only reached tiers", async () => {
      await open()
      await userEvent.click(screen.getByRole('button', { name: /^Traveler/ }))
      expect(screen.getByRole('heading', { name: 'Traveler Starter' })).toBeInTheDocument()
      expect(screen.getByText('1 scene explored')).toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Traveler Starter' })).toBeInTheDocument()
      expect(screen.queryByRole('button', { name: 'Traveler Bronze' })).not.toBeInTheDocument()
      expect(screen.queryByText(/NEXT TIER/)).not.toBeInTheDocument()
      expect(screen.queryByRole('progressbar')).not.toBeInTheDocument()
    })

    it('your own passport shows every tier, the next one to reach, and the progress toward it', async () => {
      await open(true)
      await userEvent.click(screen.getByRole('button', { name: /^Traveler/ }))
      await userEvent.click(screen.getByRole('button', { name: 'Traveler Bronze' }))
      expect(screen.getByRole('heading', { name: 'Traveler Bronze' })).toBeInTheDocument()
      expect(screen.getByText('Locked')).toBeInTheDocument()
      expect(screen.getByText('50 scenes explored', { selector: 'p' })).toBeInTheDocument()
      expect(screen.getByText('NEXT TIER: Bronze')).toBeInTheDocument()
      // The card carries the bar alone; the detail adds the count.
      expect(screen.getAllByText('5/50')).toHaveLength(1)
      expect(screen.getAllByRole('progressbar')).toHaveLength(2)
    })
  })

  it('shows a graceful empty state when the user has no details', () => {
    render(<ProfilePassport profile={{ address: '0xnobody', name: 'Nobody', hasClaimedName: false, isGuest: false }} onClose={vi.fn()} />)
    expect(screen.getByText(/no details to show/i)).toBeInTheDocument()
  })

  it('switches tabs (Photos shows its empty state)', async () => {
    setPublicReelFetcher(async () => ({ ok: true, photos: [], total: 0, max: 500 }))
    render(<ProfilePassport profile={profile} onClose={vi.fn()} />)
    await userEvent.click(screen.getByRole('tab', { name: 'PHOTOS' }))
    expect(await screen.findByText('No photos here yet!')).toBeInTheDocument()
  })
})

// Equipped Wearables / Equipped Emotes — the passport's read-only item grid (unity-explorer's
// EquippedItems module). The rules pinned here: what earns a tile, what earns a SHOP link, and the
// section order the Overview tab is supposed to follow.
const SHOP_TIARA = 'https://decentraland.org/shop/item/0xc0ffee/3'
const SHOP_DISCO = 'https://decentraland.org/shop/item/0xdecade/1'

const TIARA: Wearable = {
  urn: 'urn:decentraland:matic:collections-v2:0xc0ffee:3',
  name: 'Neon Tiara', rarity: 'legendary', category: 'tiara', equipped: true, shopUrl: SHOP_TIARA
}
const BASE_HAIR: Wearable = {
  urn: 'urn:decentraland:off-chain:base-avatars:casual_hair_01',
  name: 'Casual Hair', rarity: 'base', category: 'hair', equipped: true // off-chain: no listing
}
const BODY_SHAPE: Wearable = {
  urn: 'urn:decentraland:off-chain:base-avatars:BaseFemale',
  name: 'Base Female', rarity: 'base', category: 'body_shape', equipped: true
}
const DISCO: Emote = { slot: 0, urn: 'urn:decentraland:matic:collections-v2:0xdecade:1', name: 'Disco', rarity: 'epic', shopUrl: SHOP_DISCO }
const WAVE: Emote = { slot: 1, urn: 'urn:decentraland:off-chain:base-emotes:wave', name: 'Wave', rarity: 'base' }

const equippedProfile: Profile = {
  ...profile,
  links: undefined,
  equippedWearables: [TIARA, BASE_HAIR, BODY_SHAPE],
  equippedEmotes: [DISCO, WAVE]
}

describe('passport equipped items', () => {
  it('renders one Equipped Items section with a tile per item, wearables then emotes', () => {
    render(<ProfilePassport profile={equippedProfile} onClose={vi.fn()} />)
    expect(screen.getByRole('heading', { name: 'Equipped Items' })).toBeInTheDocument()
    for (const name of ['Neon Tiara', 'Casual Hair', 'Disco', 'Wave']) {
      expect(screen.getByText(name)).toBeInTheDocument()
    }
  })

  it('shows a SHOP link only for on-chain collectibles', () => {
    render(<ProfilePassport profile={equippedProfile} onClose={vi.fn()} />)
    // One per collectible (the tiara + the Disco emote); the base hair and base emote get none.
    const shop = screen.getAllByRole('link', { name: 'Shop' })
    expect(shop.map((a) => a.getAttribute('href')).sort()).toEqual([SHOP_TIARA, SHOP_DISCO].sort())
    for (const a of shop) {
      expect(a).toHaveAttribute('target', '_blank')
      expect(a).toHaveAttribute('rel', 'noopener noreferrer')
    }
  })

  it('drops the body shape (Unity skips BODY_SHAPE before filling the grid)', () => {
    render(<ProfilePassport profile={equippedProfile} onClose={vi.fn()} />)
    expect(screen.queryByText('Base Female')).toBeNull()
  })

  // The same emote can sit in two wheel slots (the emote list isn't deduped, unlike the wearable
  // set). Both tiles render either way on the first pass, so the thing to pin is that they carry
  // DISTINCT keys — with a duplicate key React's reconciliation of this list is undefined.
  it('gives the same emote in two wheel slots distinct keys', () => {
    const warn = vi.spyOn(console, 'error').mockImplementation(() => {})
    const twice: Emote[] = [{ ...WAVE, slot: 1 }, { ...WAVE, slot: 4 }]
    render(<ProfilePassport profile={{ ...equippedProfile, equippedEmotes: twice }} onClose={vi.fn()} />)
    expect(screen.getAllByText('Wave')).toHaveLength(2)
    expect(warn.mock.calls.flat().join(' ')).not.toMatch(/same key/i)
    warn.mockRestore()
  })

  it('orders the Overview as About Me → Equipped → Badges', () => {
    render(<ProfilePassport profile={equippedProfile} onClose={vi.fn()} />)
    const titles = screen.getAllByRole('heading', { level: 2 }).map((h) => h.textContent)
    expect(titles).toEqual(['About Me', 'Equipped Items', 'Badges'])
  })

  it('keeps the section while either kind is equipped', () => {
    render(<ProfilePassport profile={{ ...equippedProfile, equippedEmotes: [] }} onClose={vi.fn()} />)
    expect(screen.getByRole('heading', { name: 'Equipped Items' })).toBeInTheDocument()
    expect(screen.queryByText('Disco')).toBeNull()
  })

  it('counts as Overview content on its own (no empty state)', () => {
    const onlyEquipped: Profile = {
      address: '0xnobody', name: 'Nobody', hasClaimedName: false, isGuest: false,
      equippedWearables: [TIARA]
    }
    render(<ProfilePassport profile={onlyEquipped} onClose={vi.fn()} />)
    expect(screen.queryByText(/no details to show/i)).toBeNull()
    expect(screen.getByText('Neon Tiara')).toBeInTheDocument()
  })

  it('a body-shape-only equipped set leaves no section behind', () => {
    render(<ProfilePassport profile={{ ...equippedProfile, equippedWearables: [BODY_SHAPE], equippedEmotes: [] }} onClose={vi.fn()} />)
    expect(screen.queryByRole('heading', { name: 'Equipped Items' })).toBeNull()
  })
})

describe('passport photos', () => {
  const reel: GalleryPhoto[] = Array.from({ length: 3 }, (_, i) => ({ id: `p${i}`, url: `https://img/${i}.jpg`, thumbnailUrl: `https://img/${i}_t.jpg`, dateTime: '1707000000', isPublic: true }))
  const actions = (): PassportPhotoActions => ({
    setPublic: vi.fn(async () => ({ ok: true })),
    remove: vi.fn(async () => ({ ok: true })),
    metas: {},
    loadPhoto: vi.fn(),
    loadWearables: vi.fn(async () => []),
    onJumpIn: vi.fn(() => true)
  })
  async function openPhotos(isSelf: boolean, a = actions()): Promise<PassportPhotoActions> {
    const fetcher = vi.fn(async (_address: string, offset: number, limit: number) => ({ ok: true, photos: reel.slice(offset, offset + limit), total: reel.length, max: 500 }))
    setPublicReelFetcher(fetcher)
    render(
      <>
        <ProfilePassport profile={profile} isSelf={isSelf} onClose={vi.fn()} photoActions={a} />
        <PopupHost />
      </>
    )
    await userEvent.click(screen.getByRole('tab', { name: 'PHOTOS' }))
    const buttons = await screen.findAllByRole('button', { name: 'Open photo' })
    for (const b of buttons) fireEvent.load(b.querySelector('img') as HTMLImageElement)
    expect(fetcher).toHaveBeenCalledWith(profile.address.toLowerCase(), 0, 100)
    return a
  }

  it('lists the public reel; another player\'s photos have no options', async () => {
    await openPhotos(false)
    expect(screen.getAllByRole('button', { name: 'Open photo' })).toHaveLength(3)
    expect(screen.queryByRole('button', { name: 'Photo options' })).toBeNull()
  })

  it('the photo viewer opens outside the passport panel', async () => {
    await openPhotos(false)
    await userEvent.click(screen.getAllByRole('button', { name: 'Open photo' })[0])
    expect(screen.getByRole('dialog', { name: 'Photo' }).closest('[data-tab]')).toBeNull()
  })

  it.each([
    ['your own', true],
    ["another player's", false]
  ])('an empty reel on %s passport: the public-photos line shows only to the owner', async (_whose, isSelf) => {
    setPublicReelFetcher(async () => ({ ok: true, photos: [], total: 0, max: 500 }))
    render(<ProfilePassport profile={profile} isSelf={isSelf} onClose={vi.fn()} />)
    await userEvent.click(screen.getByRole('tab', { name: 'PHOTOS' }))
    expect(await screen.findByText('No photos here yet!')).toBeInTheDocument()
    expect(screen.queryAllByText(/Set your photos as public/)).toHaveLength(isSelf ? 1 : 0)
  })

  it('your own photos have every option, Delete included', async () => {
    await openPhotos(true)
    await userEvent.click(screen.getAllByRole('button', { name: 'Photo options' })[0])
    const menu = screen.getByRole('menu', { name: 'Photo options' })
    expect(within(menu).getByRole('menuitemcheckbox', { name: 'Set as Public' })).toHaveAttribute('aria-checked', 'true')
    expect(within(menu).getAllByRole('menuitem').map((b) => b.textContent)).toEqual(['Share on X', 'Copy Link', 'Download', 'Delete'])
  })

  it('Delete from the grid asks first, then deletes and drops the photo', async () => {
    const a = await openPhotos(true)
    await userEvent.click(screen.getAllByRole('button', { name: 'Photo options' })[1])
    await userEvent.click(within(screen.getByRole('menu', { name: 'Photo options' })).getByRole('menuitem', { name: 'Delete' }))
    expect(a.remove).not.toHaveBeenCalled()
    await userEvent.click(screen.getByRole('button', { name: 'DELETE' }))
    expect(a.remove).toHaveBeenCalledWith('p1')
    await waitFor(() => expect(screen.getAllByRole('button', { name: 'Open photo' })).toHaveLength(2))
  })

  it('making a photo private takes it off the grid at once and closes its menu', async () => {
    const a = await openPhotos(true)
    await userEvent.click(screen.getAllByRole('button', { name: 'Photo options' })[0])
    await userEvent.click(screen.getByRole('menuitemcheckbox', { name: 'Set as Public' }))
    expect(a.setPublic).toHaveBeenCalledWith('p0', false)
    await waitFor(() => expect(screen.getAllByRole('button', { name: 'Open photo' })).toHaveLength(2))
    expect(screen.queryByRole('menu', { name: 'Photo options' })).toBeNull()
  })

  it('a failed private write puts the photo back in its place', async () => {
    const a = actions()
    a.setPublic = vi.fn(async () => ({ ok: false }))
    await openPhotos(true, a)
    await userEvent.click(screen.getAllByRole('button', { name: 'Photo options' })[1])
    await userEvent.click(screen.getByRole('menuitemcheckbox', { name: 'Set as Public' }))
    expect(a.setPublic).toHaveBeenCalledWith('p1', false)
    await waitFor(() => expect(screen.getAllByRole('button', { name: 'Open photo' })).toHaveLength(3))
    const srcs = screen.getAllByRole('button', { name: 'Open photo' }).map((b) => b.querySelector('img')?.getAttribute('src'))
    expect(srcs).toEqual(['https://img/0_t.jpg', 'https://img/1_t.jpg', 'https://img/2_t.jpg'])
  })

  it('a thumbnail opens the photo viewer', async () => {
    const a = await openPhotos(false)
    await userEvent.click(screen.getAllByRole('button', { name: 'Open photo' })[1])
    expect(screen.getByRole('dialog', { name: 'Photo' })).toBeInTheDocument()
    expect(a.loadPhoto).toHaveBeenCalledWith('p1')
    expect(screen.queryByRole('button', { name: /Delete/ })).toBeNull()
  })
})

describe('when the passport can play an equipped emote on its avatar', () => {
  it('should play the emote of the tile that is clicked, by its picture button or anywhere on the tile', async () => {
    const onPlayEmote = vi.fn()
    render(<ProfilePassport profile={equippedProfile} onPlayEmote={onPlayEmote} onClose={vi.fn()} />)
    await userEvent.click(screen.getByRole('button', { name: 'Play Disco' }))
    expect(onPlayEmote).toHaveBeenLastCalledWith(DISCO.urn)
    await userEvent.click(screen.getByText('Wave'))
    expect(onPlayEmote).toHaveBeenLastCalledWith(WAVE.urn)
  })

  it('should leave the tile\'s shop link to open the shop alone', async () => {
    const onPlayEmote = vi.fn()
    render(<ProfilePassport profile={equippedProfile} onPlayEmote={onPlayEmote} onClose={vi.fn()} />)
    const shop = screen.getAllByRole('link', { name: /shop/i }).find((link) => link.getAttribute('href') === SHOP_DISCO)!
    shop.addEventListener('click', (e) => e.preventDefault())
    await userEvent.click(shop)
    expect(onPlayEmote).not.toHaveBeenCalled()
  })

  it('should give the wearable tiles and, without it, the emote tiles no button', () => {
    const { unmount } = render(<ProfilePassport profile={equippedProfile} onPlayEmote={vi.fn()} onClose={vi.fn()} />)
    expect(screen.queryByRole('button', { name: /Play Neon Tiara/ })).toBeNull()
    unmount()
    render(<ProfilePassport profile={equippedProfile} onClose={vi.fn()} />)
    expect(screen.queryByRole('button', { name: 'Play Disco' })).toBeNull()
  })
})
