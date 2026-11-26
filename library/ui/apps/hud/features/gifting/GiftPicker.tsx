// The gift picker: "Send a Gift to <name>" with the recipient's face and
// address, a search, WEARABLES and EMOTES tabs over the player's owned on-chain items (newest first, 16 a page, more
// as the grid scrolls), and a footer that warns the gift can't be undone or names what is about to be sent. SEND GIFT
// hands the chosen token to the host (`@hud-host`'s `gifting`), which checks it, signs it and shows its progress.
//
// Rules (explorer-core features/gifting/picker): pending gifts don't count as copies, an equipped item can be picked
// only with a spare copy, and the token sent is the newest-transferred one neither equipped nor pending. An item that
// isn't a Polygon collection item is refused: a gift is a gasless Polygon meta-transaction.

import { Button, Spinner } from '@dcl/one-ui'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { gifting } from '@hud-host'
import type { CatalogKind, GiftableItem, GiftingPageMessage } from '@explorer-core/engine/protocol'
import { GIFT_PAGE_SIZE, NOT_TRANSFERABLE, DEFAULT_INFO, bestToken, giftRows, transferable } from '@explorer-core/features/gifting/picker'
import { Avatar, Close, ControlButton, Copy, Modal, SearchField, Tabs, WearableCard, openPopup, rarityOf, showToast, type TabItem } from '../../design'
import { useSession } from '../session/SessionContext'
import { knownUserColor, useProfile } from '../session/profileStore'
import { CategoryIcon } from '../backpack/categoryIcons'
import { LoadMore } from '../communities/LoadMore'
import { usePendingGifts } from './useGifting'
import styles from './GiftPicker.module.css'

const SEARCH_DEBOUNCE_MS = 500
/** Shown when the host refused or didn't answer: the picker is already closed. */
const OPEN_FAILED = 'There was an error while trying to process your request. Please try again!'
const TABS: TabItem<CatalogKind>[] = [
  { id: 'wearables', label: 'WEARABLES' },
  { id: 'emotes', label: 'EMOTES' }
]

interface GiftRecipient {
  address: string
  name: string
}

/** Opens the picker for a recipient. */
export function openGiftPicker(recipient: GiftRecipient): void {
  openPopup((close) => <GiftPicker recipient={recipient} onClose={close} />)
}

type Grid = { items: GiftableItem[]; total: number; equipped: string[]; loading: boolean; failed: boolean }
const EMPTY: Grid = { items: [], total: 0, equipped: [], loading: true, failed: false }

export function GiftPicker({ recipient, onClose }: { recipient: GiftRecipient; onClose: () => void }): React.JSX.Element {
  const { queryGifting } = useSession()
  const profile = useProfile(recipient.address)
  const pending = usePendingGifts()
  const [tab, setTab] = useState<CatalogKind>('wearables')
  const [search, setSearch] = useState('')
  const [query, setQuery] = useState('')
  const [grid, setGrid] = useState<Grid>(EMPTY)
  const [selected, setSelected] = useState<string | null>(null)
  const generation = useRef(0)
  const loadingMore = useRef(false)

  const name = profile?.name || recipient.name
  const color = knownUserColor(recipient.address, name)


  // The search applies half a second after the last keystroke.
  useEffect(() => {
    const t = setTimeout(() => setQuery(search), SEARCH_DEBOUNCE_MS)
    return () => clearTimeout(t)
  }, [search])

  const read = useCallback(
    async (page: number, reset: boolean) => {
      const gen = reset ? ++generation.current : generation.current
      if (reset) {
        setGrid(EMPTY)
        setSelected(null)
      } else setGrid((g) => ({ ...g, loading: true }))
      const answer: GiftingPageMessage = await queryGifting({ catalog: tab, page, pageSize: GIFT_PAGE_SIZE, ...(query ? { search: query } : {}) })
      if (gen !== generation.current) return
      setGrid((g) => {
        const known = new Set(g.items.map((i) => i.urn))
        const items = reset ? answer.items : [...g.items, ...answer.items.filter((i) => !known.has(i.urn))]
        return { items, total: answer.total, equipped: answer.equipped, loading: false, failed: answer.failed && items.length === 0 }
      })
      loadingMore.current = false
    },
    [queryGifting, tab, query]
  )

  useEffect(() => {
    void read(0, true)
  }, [read])

  const rows = useMemo(() => giftRows(grid.items, grid.equipped, pending), [grid.items, grid.equipped, pending])
  const chosen = rows.find((r) => r.item.urn === selected) ?? null

  const loadMore = (): void => {
    if (loadingMore.current || grid.loading || grid.items.length >= grid.total) return
    loadingMore.current = true
    void read(Math.ceil(grid.items.length / GIFT_PAGE_SIZE), false)
  }

  const send = (): void => {
    if (chosen == null || !gifting) return
    const token = transferable(chosen.item.tokens[0]?.urn ?? '') ? bestToken(chosen.item, grid.equipped, pending) : null
    if (token == null) {
      showToast(NOT_TRANSFERABLE, { tone: 'error' })
      return
    }
    onClose()
    gifting
      .open({ recipient: recipient.address, urn: token.urn, tokenId: token.tokenId, transferredAt: token.transferredAt })
      .catch((e: unknown) => {
        console.warn('[gifting] could not open', e)
        showToast(OPEN_FAILED, { tone: 'error' })
      })
  }

  const empty = !grid.loading && rows.length === 0

  return (
    <Modal width={880} ariaLabel={`Send a Gift to ${name}`} className={styles.modal}>
      <div className={styles.header}>
        <Avatar src={profile?.picture} name={name} color={color} size={48} />
        <div className={styles.who}>
          <h2 className={styles.title}>
            Send a Gift to <span style={{ color }}>{name}</span>
          </h2>
          <button type="button" className={styles.address} title="Copy address" onClick={() => void navigator.clipboard?.writeText(recipient.address).catch(() => {})}>
            {`${recipient.address.slice(0, 6)}…${recipient.address.slice(-4)}`}
            <Copy size={12} />
          </button>
        </div>
        <div className={styles.search}>
          <SearchField value={search} onChange={setSearch} onSubmit={(v) => setQuery(v)} clearable onClear={() => setQuery('')} />
        </div>
        <ControlButton variant="dark" aria-label="Close" onClick={onClose}>
          <Close size={18} />
        </ControlButton>
      </div>
      <Tabs
        items={TABS}
        value={tab}
        onChange={(t) => {
          setTab(t)
          setSearch('')
          setQuery('')
        }}
        variant="underline"
        aria-label="Gift type"
      />
      <div className={styles.grid} role="group" aria-label={tab === 'wearables' ? 'Wearables' : 'Emotes'}>
        {grid.loading && grid.items.length === 0 && (
          <div className={styles.centre}>
            <Spinner variant="thin" />
          </div>
        )}
        {empty && <p className={styles.centre}>No assets match your search terms.</p>}
        {rows.map((r) => (
          <div key={r.item.urn} className={styles.cell}>
            <WearableCard
              pick
              name={r.item.name}
              thumbnail={r.item.thumbnail}
              rarity={rarityOf(r.item.rarity)}
              categoryIcon={r.item.category ? <CategoryIcon category={r.item.category} size={20} /> : undefined}
              count={r.amount}
              selected={selected === r.item.urn}
              coverNote={r.giftable ? undefined : 'Equipped'}
              onClick={() => setSelected((s) => (s === r.item.urn ? null : r.item.urn))}
            />
          </div>
        ))}
        {grid.items.length > 0 && grid.items.length < grid.total && <LoadMore count={grid.items.length} onVisible={loadMore} />}
      </div>
      <div className={styles.footer}>
        <p className={styles.info}>
          {chosen ? (
            <>
              You are about to send <b>{chosen.item.name}</b> to <b>{name}</b>
            </>
          ) : (
            DEFAULT_INFO
          )}
        </p>
        <Button appearance="hud" variant="secondary" onClick={onClose}>
          CANCEL
        </Button>
        <Button appearance="hud" disabled={chosen == null} onClick={send}>
          SEND GIFT
        </Button>
      </div>
    </Modal>
  )
}
