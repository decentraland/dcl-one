import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest'
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import type { GiftableItem, GiftingPageMessage } from '@explorer-core/engine/protocol'
import type { HostGifting } from '@explorer-core/hostContract'
import { PopupHost, ToastHost, resetPopups } from '../../design'
import { GiftPicker } from '../../features/gifting/GiftPicker'
import { SessionProvider } from '../../features/session/SessionContext'
import { fakeSession } from '../harness'

const RECIPIENT = { address: '0x' + '2'.repeat(40), name: 'Bob' }
const MATIC = 'urn:decentraland:matic:collections-v2:0xabc:0'
const ETH = 'urn:decentraland:ethereum:collections-v1:dcl_launch:hat'

const item = (urn: string, name: string, amount = 1): GiftableItem => ({
  urn,
  name,
  rarity: 'epic',
  category: 'hat',
  amount,
  tokens: Array.from({ length: amount }, (_, i) => ({ urn: `${urn}:${i + 7}`, tokenId: String(i + 7), transferredAt: 100 + i }))
})
const page = (items: GiftableItem[], equipped: string[] = []): GiftingPageMessage => ({ kind: 'giftingPage', catalog: 'wearables', items, total: items.length, equipped, failed: false, requestId: 1 })

let host: HostGifting & { open: ReturnType<typeof vi.fn> }
vi.mock('@hud-host', async (importOriginal) => ({
  ...(await importOriginal<object>()),
  get gifting() {
    return host
  }
}))

beforeEach(() => {
  host = { available: async () => true, pending: async () => [], open: vi.fn(async () => undefined), onSent: () => () => undefined }
})
afterEach(() => {
  act(() => resetPopups())
})

function show(answer: GiftingPageMessage): { session: ReturnType<typeof fakeSession>; onClose: ReturnType<typeof vi.fn> } {
  const session = fakeSession()
  session.queryGifting = vi.fn(async () => answer)
  const onClose = vi.fn()
  render(
    <SessionProvider value={session}>
      <GiftPicker recipient={RECIPIENT} onClose={onClose} />
      <PopupHost />
      <ToastHost />
    </SessionProvider>
  )
  return { session, onClose }
}

describe('the gift picker', () => {
  it('asks for the first page of on-chain wearables and shows them', async () => {
    const { session } = show(page([item(MATIC, 'Cool Hat', 3)]))
    expect(await screen.findByRole('button', { name: 'Cool Hat' })).toBeTruthy()
    expect(session.queryGifting).toHaveBeenCalledWith({ catalog: 'wearables', page: 0, pageSize: 16 })
    expect(screen.getByText('x3')).toBeTruthy()
    expect(screen.getByText('Gifting an item cannot be undone.')).toBeTruthy()
    expect((screen.getByRole('button', { name: 'SEND GIFT' }) as HTMLButtonElement).disabled).toBe(true)
  })

  it('names what is about to be sent once an item is picked, and sends its token to the host', async () => {
    const { onClose } = show(page([item(MATIC, 'Cool Hat', 2)]))
    fireEvent.click(await screen.findByRole('button', { name: 'Cool Hat' }))
    expect(screen.getByText(/You are about to send/).textContent).toBe('You are about to send Cool Hat to Bob')
    fireEvent.click(screen.getByRole('button', { name: 'SEND GIFT' }))
    expect(onClose).toHaveBeenCalled()
    expect(host.open).toHaveBeenCalledWith({ recipient: RECIPIENT.address, urn: `${MATIC}:8`, tokenId: '8', recipientName: 'Bob', transferredAt: 101 })
  })

  it('covers an equipped single copy, which cannot be picked', async () => {
    show(page([item(MATIC, 'Cool Hat')], [`${MATIC}:7`]))
    const card = await screen.findByRole('button', { name: 'Cool Hat' })
    expect(screen.getByText('Equipped')).toBeTruthy()
    fireEvent.click(card)
    expect((screen.getByRole('button', { name: 'SEND GIFT' }) as HTMLButtonElement).disabled).toBe(true)
  })

  it("refuses an item that isn't a Polygon collection token", async () => {
    const { onClose } = show(page([item(ETH, 'Old Hat')]))
    fireEvent.click(await screen.findByRole('button', { name: 'Old Hat' }))
    fireEvent.click(screen.getByRole('button', { name: 'SEND GIFT' }))
    expect(await screen.findByText("Cannot send this item as a gift because it's not a transferable NFT.")).toBeTruthy()
    expect(host.open).not.toHaveBeenCalled()
    expect(onClose).not.toHaveBeenCalled()
  })

  it('hides a gift the host says is pending', async () => {
    host.pending = async () => [{ urn: `${MATIC}:7`, transferredAt: 100 }]
    show(page([item(MATIC, 'Cool Hat'), item(`${MATIC.slice(0, -1)}1`, 'Other Hat')]))
    expect(await screen.findByRole('button', { name: 'Other Hat' })).toBeTruthy()
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Cool Hat' })).toBeNull())
  })

  it('says when nothing matches, and reads the emotes on their tab', async () => {
    const { session } = show(page([]))
    expect(await screen.findByText('No assets match your search terms.')).toBeTruthy()
    fireEvent.click(screen.getByRole('tab', { name: 'EMOTES' }))
    await waitFor(() => expect(session.queryGifting).toHaveBeenLastCalledWith({ catalog: 'emotes', page: 0, pageSize: 16 }))
  })
})
