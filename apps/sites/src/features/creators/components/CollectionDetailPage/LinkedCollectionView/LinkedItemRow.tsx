import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from '@/data/lib/creators/intl'
import { getContentsStorageUrl } from '@/data/lib/creators/builder'
import type { Item } from '@/data/lib/creators/items'
import type { LinkedItemStatus } from '@/data/lib/creators/linkedItemStatus'
import { areMappingsEqual, buildItemMappings, getItemMapping, type Mapping, type Mappings } from '@/data/lib/creators/mappings'
import { decodeLinkedUrn, type LinkedContract } from '@/data/lib/creators/thirdParty'
import { CategoryIcon } from '../../ItemIcons'
import { ItemThumbnail } from '../../ItemThumbnail'
import { MappingEditor } from '../../MappingEditor'
import pill from '../../CollectionStatusPill/CollectionStatusPill.module.css'
import linked from './LinkedCollectionView.module.css'
import row from '../ItemListRow/ItemListRow.module.css'
import shared from '../../../styles/shared.module.css'

const EMPTY = '—'
const MAPPING_SAVE_DEBOUNCE_MS = 1000
let rowGenerations = 0

export type MappingStates = ReadonlyMap<string, { dirty: boolean; generation: number }>

/** The rows' mapping states after one reports: a row already gone (an older generation) never overrules a newer one. */
export function nextMappingStates(
  current: MappingStates,
  itemId: string,
  dirty: boolean,
  generation: number
): MappingStates {
  const known = current.get(itemId)
  if (known && (known.generation > generation || (known.generation === generation && known.dirty === dirty))) {
    return current
  }
  return new Map(current).set(itemId, { dirty, generation })
}

type Props = {
  item: Item
  contract: LinkedContract | null
  /** The row's status; undefined while the item's curation is still loading. */
  status: LinkedItemStatus | undefined
  /** Whether the row can be selected for publishing; its checkbox is otherwise disabled. */
  selectable: boolean
  selected: boolean
  onSelectedChange: (selected: boolean) => void
  /** Whether the viewer may edit the mapping in place (desktop managers). */
  editable: boolean
  /** Saves a new mapping; a rejected promise keeps the row's draft so the creator can retry. */
  onSaveMappings: (item: Item, mappings: Mappings) => Promise<unknown>
  /** Whether a mapping edit is waiting for the pause or still saving. */
  /**
   * Whether a mapping edit is waiting for the pause or still saving. `generation` grows with each row shown, so a save
   * from a row already gone can't clear a newer one's edit of the same item.
   */
  onMappingDirtyChange?: (itemId: string, dirty: boolean, generation: number) => void
  actions?: ReactNode
}

/** A linked item: thumbnail, name, category, review status and the token mapping that unlocks it. */
export function LinkedItemRow({
  item,
  contract,
  status,
  selectable,
  selected,
  onSelectedChange,
  editable,
  onSaveMappings,
  onMappingDirtyChange,
  actions
}: Props) {
  const { t } = useTranslation()
  const thumbnailHash = item.contents[item.thumbnail]
  const category = item.data.category
  const mapping = getItemMapping(item.mappings, contract)
  const [isSaving, setSaving] = useState(false)
  // The mapping the last save failed to store, offered again with a retry until a save of it (or a newer edit) lands.
  const [failedSave, setFailedSave] = useState<Mappings | null>(null)
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)
  // The edit waiting for the pause, so leaving the row (page, filter, editor) saves it instead of dropping it.
  const pending = useRef<Mappings | null>(null)
  // What the editor holds (null while invalid) and what the server last confirmed, which the item lags after a save.
  const draft = useRef<Mappings | null | undefined>(undefined)
  const saved = useRef(item.mappings)
  const inFlight = useRef(0)
  const mounted = useRef(true)
  const generation = useRef(++rowGenerations)
  const latest = useRef({ item, onSaveMappings, onMappingDirtyChange })
  latest.current = { item, onSaveMappings, onMappingDirtyChange }
  const setDirty = (dirty: boolean) =>
    latest.current.onMappingDirtyChange?.(latest.current.item.id, dirty, generation.current)

  useEffect(() => {
    saved.current = item.mappings
  }, [item.mappings])

  // Clean only once the server holds what the editor shows: a failed save, an invalid draft or a newer edit keep the
  // row unsaved, so publishing can't go out with another mapping than the one on screen.
  const save = useCallback(async (mappings: Mappings): Promise<void> => {
    inFlight.current++
    try {
      await latest.current.onSaveMappings(latest.current.item, mappings)
      saved.current = mappings
      if (mounted.current) setFailedSave(null)
    } catch {
      if (mounted.current && !pending.current) setFailedSave(mappings)
      return
    } finally {
      inFlight.current--
    }
    // Gone while this saved: with nothing newer waiting (the cleanup saves that itself), the row is clean.
    if (!mounted.current) {
      if (!pending.current && inFlight.current === 0) setDirty(false)
      return
    }
    if (pending.current || inFlight.current > 0 || draft.current === null) return
    if (!draft.current || areMappingsEqual(draft.current, mappings)) return setDirty(false)
    // The editor moved on while this was saving: save what it shows now.
    pending.current = draft.current
    flushRef.current()
  }, [])

  const flush = useCallback(() => {
    clearTimeout(timer.current)
    const mappings = pending.current
    pending.current = null
    if (!mappings) return
    setSaving(true)
    void save(mappings).finally(() => {
      if (mounted.current) setSaving(false)
    })
  }, [save])
  const flushRef = useRef(flush)
  flushRef.current = flush

  useEffect(() => {
    // Set here, not only at creation: StrictMode runs this cleanup once and mounts again.
    mounted.current = true
    // A row shows the server's mapping when it (re)appears: nothing of it is unsaved yet.
    latest.current.onMappingDirtyChange?.(latest.current.item.id, false, generation.current)
    return () => {
      mounted.current = false
      clearTimeout(timer.current)
      const mappings = pending.current
      const { item: current, onSaveMappings: persist, onMappingDirtyChange: report } = latest.current
      // An edit still waiting is saved on the way out; only once it lands is the row clean. A failure keeps Publish
      // blocked (its toast names the item) until the row is shown again with the server's mapping.
      if (mappings) {
        void persist(current, mappings).then(
          () => report?.(current.id, false, generation.current),
          () => undefined
        )
      } else if (draft.current === null) {
        report?.(current.id, false, generation.current)
      }
    }
  }, [save])

  // A valid change saves itself after a pause, like the legacy row: no save button per mapping.
  function onMappingChange(next: Mapping | null) {
    clearTimeout(timer.current)
    const mappings = next && contract ? buildItemMappings(next, contract) : null
    draft.current = mappings
    pending.current = null
    setFailedSave(null)
    if (!mappings) return setDirty(true)
    if (inFlight.current === 0 && areMappingsEqual(mappings, saved.current)) return setDirty(false)
    pending.current = mappings
    setDirty(true)
    timer.current = setTimeout(flush, MAPPING_SAVE_DEBOUNCE_MS)
  }

  return (
    <article className={linked.row} data-testid="linked-item-row" data-selected={selected || undefined}>
      <div className={linked.selectCell}>
        <input
          type="checkbox"
          checked={selected}
          disabled={!selectable}
          aria-label={t('linked_collection.select_item', { name: item.name })}
          data-testid="linked-item-select"
          onChange={event => onSelectedChange(event.target.checked)}
        />
      </div>
      <div className={row.thumb}>
        <ItemThumbnail src={thumbnailHash ? getContentsStorageUrl(thumbnailHash) : null} rarity={item.rarity} />
      </div>
      <div className={linked.nameCell}>
        <span className={row.nameText} data-testid="item-row-name" title={item.name}>
          {item.name}
        </span>
        {item.urn && (
          <span className={linked.urn} title={item.urn} data-testid="linked-item-urn">
            {decodeLinkedUrn(item.urn)?.itemSuffix ?? item.urn}
          </span>
        )}
      </div>
      <div className={row.cell} data-testid="item-row-category">
        {category ? <CategoryIcon category={category} withLabel /> : EMPTY}
      </div>
      <div className={row.cell} data-testid="linked-item-status-cell">
        {status ? (
          <span className={pill.pill} data-testid="linked-item-status" data-status={status}>
            {t(`linked_collection.item_status.${status}`)}
          </span>
        ) : (
          EMPTY
        )}
      </div>
      <div className={linked.mappingCell} data-testid="linked-item-mapping" data-saving={isSaving || undefined}>
        {contract ? (
          <MappingEditor
            value={mapping}
            compact
            withHelp={false}
            groupLabel={t('linked_collection.mapping_for', { name: item.name })}
            disabled={!editable}
            testId="linked-item-mapping-editor"
            onChange={onMappingChange}
          />
        ) : (
          t('linked_collection.mapping.none')
        )}
        {isSaving && <span className={shared.spinner} data-testid="linked-item-mapping-saving" aria-hidden />}
        {failedSave && !isSaving && (
          <button
            className={linked.statusRetry}
            type="button"
            aria-label={t('linked_collection.mapping_retry_for', { name: item.name })}
            data-testid="linked-item-mapping-retry"
            onClick={() => {
              pending.current = failedSave
              setFailedSave(null)
              flush()
            }}
          >
            {t('linked_collection.mapping_retry')}
          </button>
        )}
      </div>
      <div className={row.actionsCell}>{actions}</div>
    </article>
  )
}
