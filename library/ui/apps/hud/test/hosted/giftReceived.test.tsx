import { describe, it, expect, afterEach, vi } from 'vitest'
import { act, render, screen, waitFor } from '@testing-library/react'
import type { AppNotification } from '@explorer-core/engine/protocol'
import type { GiftItem } from '@explorer-core/features/gifting/giftItem'
import { PopupHost, resetPopups } from '../../design'
import { GiftReceivedPopup, giftOf, openGiftReceived, setGiftBackpackOpener } from '../../features/gifting/GiftReceivedPopup'
import { giftTarget, summarize } from '../../features/notifications/summary'

const SENDER = '0x1234567890abcdef1234567890abcdef12345678'
const URI = 'https://peer.example.com/lambdas/collections/standard/erc721/137/0xabc/1/2'

const gift = (metadata: Record<string, unknown> = {}): AppNotification => ({
  id: 'g1',
  type: 'transfer_received',
  timestamp: String(Date.now()),
  read: false,
  metadata: { senderAddress: SENDER, receiverAddress: '0x' + '9'.repeat(40), tokenUri: URI, ...metadata }
})

const ITEM: GiftItem = { name: 'Cool Hat', description: '', imageUrl: 'https://example.com/t.png', rarity: 'epic', category: 'hat', kind: 'wearable' }

afterEach(() => {
  act(() => resetPopups())
  setGiftBackpackOpener(null)
})

describe('a gift notification', () => {
  it('reads "Gift received", naming the sender by their short address until their profile is known', () => {
    expect(summarize(gift())).toMatchObject({ title: 'Gift received', body: "0x1234…5678 sent you something! It's on its way to your Backpack." })
  })

  it('carries the sender and the token uri; other types and senders that are not addresses carry nothing', () => {
    expect(giftOf(gift())).toEqual({ senderAddress: SENDER, tokenUri: URI })
    expect(giftOf(gift({ senderAddress: 'nope' }))).toBeNull()
    expect(giftOf({ ...gift(), type: 'tip_received' })).toBeNull()
    expect(giftTarget({ ...gift(), type: 'tip_received' })).toBeUndefined()
  })
})

describe('the gift-received popup', () => {
  it('shows the item it loads and the sender', async () => {
    render(<GiftReceivedPopup gift={{ senderAddress: SENDER, tokenUri: URI }} onClose={() => undefined} onOpenBackpack={() => undefined} load={async () => ITEM} />)
    expect(screen.getByText('Loading...')).toBeTruthy()
    expect(screen.getByRole('dialog').textContent).toContain('0x1234…5678')
    expect(await screen.findByText('Cool Hat')).toBeTruthy()
    expect(screen.getByText('ON ITS WAY TO YOUR BACKPACK')).toBeTruthy()
    expect(screen.getByRole('dialog').getAttribute('data-rarity')).toBe('epic')
  })

  it('names an item it could not load Unknown Item', async () => {
    render(<GiftReceivedPopup gift={{ senderAddress: SENDER }} onClose={() => undefined} onOpenBackpack={() => undefined} load={async () => null} />)
    expect(await screen.findByText('Unknown Item')).toBeTruthy()
  })

  it('GO TO BACKPACK opens the Backpack on Emotes for an emote, and closes the popup', async () => {
    const opener = vi.fn()
    setGiftBackpackOpener(opener)
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ name: 'Dance Move', attributes: [{ trait_type: 'Category', value: 'dance' }] }), { status: 200 })
    )
    render(<PopupHost />)
    act(() => giftTarget(gift())?.())
    await screen.findByText('Dance Move')
    act(() => screen.getByRole('button', { name: 'GO TO BACKPACK' }).click())
    expect(opener).toHaveBeenCalledWith('emotes')
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    fetchSpy.mockRestore()
  })

  it('CLOSE only closes it', async () => {
    const opener = vi.fn()
    setGiftBackpackOpener(opener)
    render(<PopupHost />)
    act(() => openGiftReceived({ senderAddress: SENDER }))
    act(() => screen.getByRole('button', { name: 'CLOSE' }).click())
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    expect(opener).not.toHaveBeenCalled()
  })
})
