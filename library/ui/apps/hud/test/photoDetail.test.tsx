import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { pending } from '@explorer-core/lib/analytics'
import type { GalleryPhoto, GalleryPhotoMeta, Wearable } from '@explorer-core/engine/protocol'
import { PhotoDetail } from '../features/gallery/PhotoDetail'
import { photoDate, travelToPhoto } from '../features/gallery/reelActions'
import { receiveProfile, resetProfileStore } from '../features/session/profileStore'
import { ToastHost, resetPopups } from '../design'

const photos: GalleryPhoto[] = [
  { id: 'a', url: 'https://img/a.jpg', dateTime: '1709856000', isPublic: false },
  { id: 'b', url: 'https://img/b.jpg', dateTime: '1709856000', isPublic: true },
  { id: 'c', url: 'https://img/c.jpg', dateTime: '1709856000', isPublic: true }
]
const ALICE = '0x00000000000000000000000000000000000a11ce'
const BOB = '0x0000000000000000000000000000000000000b0b'
const meta: GalleryPhotoMeta = {
  dateTime: '1709856000',
  userName: 'Mojito',
  userAddress: '0xme',
  sceneName: 'Genesis Plaza',
  x: 0,
  y: -2,
  realm: 'main',
  people: [
    { address: ALICE, name: 'Alice', wearables: ['urn:decentraland:matic:collections-v2:0xabc:0'], official: true },
    { address: BOB, name: 'Bob', wearables: [] }
  ]
}
const tiara: Wearable = { urn: 'urn:decentraland:matic:collections-v2:0xabc:0', name: 'Neon Tiara', rarity: 'legendary', category: 'tiara', equipped: true, shopUrl: 'https://decentraland.org/shop/item/0xabc/0' }

type Props = Parameters<typeof PhotoDetail>[0]
function viewer(over: Partial<Props> = {}): Props {
  return {
    photos,
    index: 1,
    meta,
    canEdit: true,
    onIndex: vi.fn(),
    onLoadMeta: vi.fn(),
    loadWearables: vi.fn(async () => [tiara]),
    onClose: vi.fn(),
    onJumpIn: vi.fn(),
    onSetPublic: vi.fn(async () => true),
    onDelete: vi.fn(),
    onViewPerson: vi.fn(),
    ...over
  }
}

let queuedBefore = 0
const sent = (): string[] => pending().slice(queuedBefore).map((e) => e.event)

beforeEach(() => {
  queuedBefore = pending().length
  resetProfileStore()
})
afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  resetPopups()
})

describe('PhotoDetail', () => {
  it('shows the date, who took it and the place', () => {
    render(<PhotoDetail {...viewer()} />)
    const info = screen.getByRole('complementary', { name: 'Photo info' })
    expect(info).toHaveTextContent(`${photoDate(1709856000 * 1000)} - Taken by Mojito`)
    expect(within(info).getByRole('button', { name: /Jump In/ })).toHaveTextContent('Genesis Plaza, 0, -2')
    expect(sent()).toContain('open_photo')
  })

  it('hides previous at the first photo and next at the last', () => {
    const { rerender } = render(<PhotoDetail {...viewer({ index: 0 })} />)
    expect(screen.queryByRole('button', { name: 'Previous photo' })).toBeNull()
    expect(screen.getByRole('button', { name: 'Next photo' })).toBeInTheDocument()
    rerender(<PhotoDetail {...viewer({ index: 2 })} />)
    expect(screen.getByRole('button', { name: 'Previous photo' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Next photo' })).toBeNull()
  })

  it('offers the public toggle and Delete only on your own photos', () => {
    const { rerender } = render(<PhotoDetail {...viewer()} />)
    expect(screen.getByRole('switch', { name: 'Set as Public' })).toHaveAttribute('aria-checked', 'true')
    expect(screen.getByRole('button', { name: 'Delete' })).toBeInTheDocument()
    rerender(<PhotoDetail {...viewer({ canEdit: false })} />)
    expect(screen.queryByRole('switch', { name: 'Set as Public' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Delete' })).toBeNull()
    expect(screen.getAllByRole('button', { name: /Share on X|Copy Link|Download/ })).toHaveLength(3)
  })

  it('the toggle shows the choice at once and goes back when the service refuses it', async () => {
    let settle: (ok: boolean) => void = () => {}
    const onSetPublic = vi.fn(() => new Promise<boolean>((r) => (settle = r)))
    render(<PhotoDetail {...viewer({ onSetPublic })} />)
    const toggle = screen.getByRole('switch', { name: 'Set as Public' })
    await userEvent.click(toggle)
    expect(onSetPublic).toHaveBeenCalledWith(photos[1], false)
    expect(toggle).toHaveAttribute('aria-checked', 'false')
    await act(async () => settle(false))
    expect(toggle).toHaveAttribute('aria-checked', 'true')
  })

  it('a second choice stays while the first write settles, and a refused one shows the photo\'s value', async () => {
    const settle: Array<(ok: boolean) => void> = []
    const onSetPublic = vi.fn(() => new Promise<boolean>((r) => settle.push(r)))
    render(<PhotoDetail {...viewer({ onSetPublic })} />)
    const toggle = screen.getByRole('switch', { name: 'Set as Public' })
    await userEvent.click(toggle)
    await userEvent.click(toggle)
    await userEvent.click(toggle)
    expect(onSetPublic.mock.calls.map((c) => (c as unknown[])[1])).toEqual([false, true, false])
    expect(toggle).toHaveAttribute('aria-checked', 'false')
    await act(async () => settle[1](true))
    expect(toggle).toHaveAttribute('aria-checked', 'false')
    await act(async () => settle[0](true))
    expect(toggle).toHaveAttribute('aria-checked', 'false')
    await act(async () => settle[2](false))
    expect(toggle).toHaveAttribute('aria-checked', 'true')
  })

  it('the info toggle slides the panel away and back', async () => {
    render(<PhotoDetail {...viewer()} />)
    const toggle = screen.getByRole('button', { name: 'Hide photo info' })
    expect(toggle).toHaveAttribute('aria-expanded', 'true')
    await userEvent.click(toggle)
    expect(screen.getByRole('button', { name: 'Show photo info' })).toHaveAttribute('aria-expanded', 'false')
    expect(screen.getByRole('complementary', { name: 'Photo info', hidden: true })).toHaveAttribute('inert')
  })

  it('covers the panel with its shimmer until the metadata arrives', () => {
    const { container, rerender } = render(<PhotoDetail {...viewer({ meta: undefined })} />)
    expect(container.querySelectorAll('[aria-hidden="true"] > span').length).toBeGreaterThan(0)
    expect(screen.queryByRole('button', { name: /Jump In/ })).toBeNull()
    rerender(<PhotoDetail {...viewer()} />)
    expect(screen.getByRole('button', { name: /Jump In/ })).toBeInTheDocument()
  })

  it('Jump In hands over where the photo was taken', async () => {
    const onJumpIn = vi.fn()
    render(<PhotoDetail {...viewer({ onJumpIn, meta: { ...meta, realm: 'boedo.dcl.eth', x: 3, y: 4 } })} />)
    await userEvent.click(screen.getByRole('button', { name: /Jump In/ }))
    expect(onJumpIn).toHaveBeenCalledWith({ realm: 'boedo.dcl.eth', x: 3, y: 4 })
    expect(sent()).toContain('photo_jump_to')
  })

  it('the owner and each person open their passport', async () => {
    const onViewPerson = vi.fn()
    render(<PhotoDetail {...viewer({ onViewPerson })} />)
    await userEvent.click(screen.getByRole('button', { name: 'Mojito' }))
    expect(onViewPerson).toHaveBeenLastCalledWith({ address: '0xme', name: 'Mojito' })
    await userEvent.click(screen.getByRole('button', { name: 'Alice, official' }))
    expect(onViewPerson).toHaveBeenLastCalledWith(expect.objectContaining({ address: ALICE, name: 'Alice' }))
  })

  it("a person's marks: #tag for an unclaimed name, verified for a claimed one, official for an official wallet", () => {
    receiveProfile(ALICE, { address: ALICE, name: 'Alice', hasClaimedName: true, isGuest: false })
    receiveProfile(BOB, { address: BOB, name: 'Bob', hasClaimedName: false, isGuest: false })
    render(<PhotoDetail {...viewer()} />)
    const alice = screen.getByRole('button', { name: 'Alice, verified, official' })
    expect(within(alice).getByLabelText('verified')).toBeInTheDocument()
    expect(within(alice).getByLabelText('Official')).toBeInTheDocument()
    expect(alice).not.toHaveTextContent('#')
    const bob = screen.getByRole('button', { name: 'Bob' })
    expect(bob).toHaveTextContent('#0b0b')
    expect(within(bob).queryByLabelText('verified')).toBeNull()
  })

  it('a person opens the wearables they had on, read once, with BUY to the Shop', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    const open = vi.fn()
    vi.stubGlobal('open', open)
    const loadWearables = vi.fn(async () => [tiara])
    render(<PhotoDetail {...viewer({ loadWearables })} />)
    const expand = screen.getByRole('button', { name: "Show Alice's wearables" })
    await userEvent.click(expand)
    expect(loadWearables).toHaveBeenCalledWith([tiara.urn])
    const buy = await screen.findByRole('button', { name: 'Buy Neon Tiara' })
    expect(screen.getByText('Neon Tiara')).toBeInTheDocument()
    await userEvent.click(buy)
    expect(open).not.toHaveBeenCalled()
    await act(async () => vi.advanceTimersByTime(300))
    expect(open).toHaveBeenCalledWith(tiara.shopUrl, '_blank', 'noopener')
    expect(sent()).toContain('photo_to_marketplace')
    await userEvent.click(screen.getByRole('button', { name: "Hide Alice's wearables" }))
    await userEvent.click(screen.getByRole('button', { name: "Show Alice's wearables" }))
    expect(loadWearables).toHaveBeenCalledTimes(1)
    vi.useRealTimers()
  })

  it('a person with no collectibles says so', async () => {
    const loadWearables = vi.fn(async () => [])
    render(<PhotoDetail {...viewer({ loadWearables })} />)
    await userEvent.click(screen.getByRole('button', { name: "Show Bob's wearables" }))
    expect(screen.getByText('No collectibles equipped when the photo was taken')).toBeInTheDocument()
    expect(loadWearables).not.toHaveBeenCalled()
  })

  it('a guest has no passport to open and no wearables to list', () => {
    render(<PhotoDetail {...viewer({ meta: { ...meta, people: [{ address: BOB, name: 'Guest', isGuest: true }] } })} />)
    expect(screen.getByRole('button', { name: 'Guest' })).toBeDisabled()
    expect(screen.queryByRole('button', { name: "Show Guest's wearables" })).toBeNull()
  })

  it('Share on X and Download are reported', async () => {
    vi.stubGlobal('open', vi.fn())
    vi.stubGlobal('fetch', vi.fn(async () => new Response(new Blob(['jpeg']), { status: 200 })))
    URL.createObjectURL = vi.fn(() => 'blob:x')
    URL.revokeObjectURL = vi.fn()
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {})
    Object.assign(navigator, { clipboard: { writeText: vi.fn(async () => undefined) } })
    render(<PhotoDetail {...viewer()} />)
    await userEvent.click(screen.getByRole('button', { name: 'Share on X' }))
    await userEvent.click(screen.getByRole('button', { name: 'Download' }))
    await waitFor(() => expect(sent()).toEqual(expect.arrayContaining(['share_photo', 'download_photo'])))
  })

  it("the download toast keeps the folder that opens it, as in the Gallery's grid", async () => {
    const listeners = new Map<string, (payload: unknown) => void>()
    const emit = vi.fn()
    ;(window as { cef?: unknown }).cef = { emit, listen: (id: string, cb: (payload: unknown) => void) => listeners.set(id, cb) }
    vi.stubGlobal('fetch', vi.fn(async () => new Response(new Blob(['jpeg']), { status: 200 })))
    URL.createObjectURL = vi.fn(() => 'blob:cef://localhost/1')
    URL.revokeObjectURL = vi.fn()
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {})
    render(
      <>
        <PhotoDetail {...viewer()} />
        <ToastHost />
      </>
    )
    fireEvent.click(screen.getByRole('button', { name: 'Download' }))
    await vi.waitFor(() => expect(URL.createObjectURL).toHaveBeenCalled())
    act(() => listeners.get('download')?.(JSON.stringify({ url: 'blob:cef://localhost/1', ok: true, dir: '/home/ada/Downloads' })))
    const folder = await screen.findByRole('button', { name: 'Open /home/ada/Downloads' })
    await userEvent.click(folder)
    expect(emit.mock.calls).toEqual([[{ to: 'engine', kind: 'openDownloads' }]])
    delete (window as { cef?: unknown }).cef
  })
})

describe('Jump In travel', () => {
  it("goes to a world's realm, else to the Genesis parcel", () => {
    const map = { teleportToPlace: vi.fn(() => 'sent' as const), changeRealm: vi.fn(() => 'sent' as const) }
    const worlds = ['boedo.dcl.eth', 'my-world.eth', 'https://worlds.example/world/boedo.dcl.eth']
    for (const realm of worlds) travelToPhoto({ realm, x: 1, y: 2 }, map)
    expect(map.changeRealm.mock.calls).toEqual(worlds.map((realm) => [realm]))
    expect(map.teleportToPlace).not.toHaveBeenCalled()
    const others = ['main', 'https://peer.decentraland.org', undefined]
    for (const realm of others) travelToPhoto({ realm, x: 1, y: 2 }, map)
    expect(map.teleportToPlace.mock.calls).toEqual(others.map(() => [1, 2]))
    expect(map.changeRealm).toHaveBeenCalledTimes(worlds.length)
  })

  it('writes the date as month, padded day and year', () => {
    expect(photoDate(new Date(2025, 2, 8).getTime())).toBe('March 08, 2025')
  })
})
