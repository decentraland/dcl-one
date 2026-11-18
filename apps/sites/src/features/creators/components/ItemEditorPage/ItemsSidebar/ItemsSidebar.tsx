import { useEffect, useMemo, useRef } from 'react'
import type { BodyShape } from '@dcl/schemas'
import { Button } from '../../Button'
import { CollectionStatusPill } from '../../CollectionStatusPill'
import {
  EmoteIcon,
  WearableIcon,
  AddIcon,
  ArrowBackIosNewIcon as BackIcon,
  CheckCircleIcon as ReviewedIcon,
  Inventory2OutlinedIcon as RestIcon,
  RateReviewOutlinedIcon as ToReviewIcon,
  GraphicEqIcon as PlayingIcon,
  KeyboardDoubleArrowLeftIcon as CollapseIcon,
  KeyboardDoubleArrowRightIcon as ExpandIcon,
  EditIcon,
  FemaleIcon,
  MaleIcon,
  VisibilityIcon as DressedIcon,
  VisibilityOffIcon as UndressedIcon
} from '../../Icons'
import { ItemThumbnail } from '../../ItemThumbnail'
import { Tooltip } from '../../Tooltip'
import { useScrollFades } from '../../../hooks/useScrollFades'
import { useTranslation } from '@/data/lib/creators/intl'
import { track } from '@/data/lib/creators/analytics'
import { getContentsStorageUrl } from '@/data/lib/creators/builder'
import { isLinkedCollection, type Collection, type CollectionDisplayStatus } from '@/data/lib/creators/collections'
import { groupItemsByType, hasRepresentationFor, type EditorMode } from '@/data/lib/creators/itemEditor'
import { BodyShapeType, ItemType, getItemBodyShapeType, isSmartWearable, type Item } from '@/data/lib/creators/items'
import { EditorSection } from '../EditorSection'
import { cx } from '@/data/lib/creators/cx'
import styles from './ItemsSidebar.module.css'
import { Link } from 'react-router'

export type EditorBack = { to: string; label: string }

type Props = {
  collection: Collection
  /** Hidden until known, so the pill never flips once shown. */
  collectionStatus?: CollectionDisplayStatus
  items: Item[]
  isLoading: boolean
  selectedId: string | null
  dressedIds: string[]
  bodyShape: BodyShape
  isPlaying: boolean
  mode: EditorMode
  /** Narrow mode: thumbnails and icons only; the width animates between the two. */
  collapsed: boolean
  onToggleCollapsed: () => void
  /** Draft collections only; opens the add-items file picker. */
  canAddItems: boolean
  onAddItems: () => void
  /** Draft collections the viewer manages: a pencil next to the name opens the rename dialog. */
  onRename?: () => void
  onSelect: (item: Item) => void
  /** Where the back link leads and its name; a visitor with nowhere to go back to (no role, not a curator) gets none. */
  back?: EditorBack
  /** Asked before the back link leaves the editor; false keeps the user here (unsaved changes). */
  onLeave?: (to: string) => boolean
  onToggleDressed: (item: Item) => void
  /** Clicking the selected, dressed emote row toggles its playback. */
  onToggleEmotePlay: (item: Item) => void
  /**
   * A linked collection under review: splits the list into the items this review is about (new ones and pushed
   * changes) and the rest of the collection, ticking the ones the curator has opened.
   */
  review?: { pendingIds: ReadonlySet<string>; reviewedIds: ReadonlySet<string> }
  testId?: string
}

export function ItemsSidebar({
  collection,
  collectionStatus,
  items,
  isLoading,
  selectedId,
  dressedIds,
  bodyShape,
  isPlaying,
  mode,
  collapsed,
  onToggleCollapsed,
  canAddItems,
  onAddItems,
  onRename,
  onSelect,
  back,
  onLeave,
  onToggleDressed,
  onToggleEmotePlay,
  review,
  testId = 'items-sidebar'
}: Props) {
  const { t } = useTranslation()
  const listRef = useRef<HTMLDivElement>(null)
  useScrollFades(listRef, 'y')
  // A selection made elsewhere (a freshly added item, a deep link) can sit outside the scrolled list.
  useEffect(() => {
    if (!selectedId) return
    const row = listRef.current?.querySelector(`[data-testid="${testId}-row-${CSS.escape(selectedId)}"]`)
    row?.scrollIntoView?.({ block: 'nearest' })
  }, [selectedId, testId])
  const groups = useMemo(() => groupItemsByType(items), [items])
  const reviewGroups = useMemo(
    () =>
      review && review.pendingIds.size > 0
        ? {
            toReview: items.filter(item => review.pendingIds.has(item.id)),
            rest: items.filter(item => !review.pendingIds.has(item.id))
          }
        : null,
    [review, items]
  )
  const grouped = groups.wearables.length > 0 && groups.emotes.length > 0
  const showAddItems = mode === 'edit' && canAddItems

  function onRowClick(item: Item) {
    if (item.id !== selectedId) {
      // Legacy builder prop names, so the event lines up with the same one from the old item editor.
      track('Preview Item', {
        ITEM_ID: item.tokenId ?? null,
        ITEM_TYPE: item.type,
        ITEM_NAME: item.name,
        ITEM_IS_THIRD_PARTY: isLinkedCollection(collection)
      })
    }
    if (item.type === ItemType.EMOTE && item.id === selectedId && dressedIds.includes(item.id)) {
      onToggleEmotePlay(item)
      return
    }
    onSelect(item)
  }

  /** Wearables made for one body shape only; emotes and smart wearables fit every avatar. */
  function singleBodyShape(item: Item): BodyShapeType.MALE | BodyShapeType.FEMALE | null {
    if (item.type !== ItemType.WEARABLE || isSmartWearable(item)) return null
    const type = getItemBodyShapeType(item)
    return type === BodyShapeType.MALE || type === BodyShapeType.FEMALE ? type : null
  }

  function renderRow(item: Item) {
    const available = hasRepresentationFor(item, bodyShape)
    const dressed = dressedIds.includes(item.id)
    const thumbnail = item.contents[item.thumbnail]
    const playing = item.type === ItemType.EMOTE && dressed && isPlaying
    const shape = singleBodyShape(item)
    const shapeLabel = shape ? t(`item_editor.sidebar.${shape}_only`) : null
    const row = (
      <div
        className={styles.row}
        key={item.id}
        data-testid={`${testId}-row-${item.id}`}
        data-selected={item.id === selectedId || undefined}
        data-dressed={dressed || undefined}
        data-unavailable={!available || undefined}
      >
        <button
          className={styles.rowButton}
          type="button"
          aria-current={item.id === selectedId || undefined}
          aria-label={collapsed ? item.name : undefined}
          data-testid={`${testId}-select-${item.id}`}
          onClick={() => onRowClick(item)}
        >
          <span className={styles.thumb} data-dressed={dressed || undefined}>
            <ItemThumbnail src={thumbnail ? getContentsStorageUrl(thumbnail) : null} rarity={item.rarity} />
          </span>
          <span className={styles.rowName} title={item.name}>
            {item.name}
          </span>
          {review?.pendingIds.has(item.id) && (
            <>
              {/* An item already live comes back with changes; any other is new to the collection. */}
              <span className={styles.rowTag} data-testid={`${testId}-tag-${item.id}`}>
                {t(item.isApproved ? 'item_editor.sidebar.review_tag.changes' : 'item_editor.sidebar.review_tag.new')}
              </span>
              {review.reviewedIds.has(item.id) && (
                <span
                  className={styles.rowGlyph}
                  data-reviewed
                  role="img"
                  aria-label={t('item_editor.sidebar.reviewed')}
                  data-testid={`${testId}-reviewed-${item.id}`}
                >
                  <ReviewedIcon />
                </span>
              )}
            </>
          )}
          {playing && (
            <span className={styles.rowGlyph} data-playing data-testid={`${testId}-playing-${item.id}`}>
              <PlayingIcon />
            </span>
          )}
        </button>
        {/* Collapsed, the row's own tooltip names the shape; this one's trigger would still take room beside the thumb. */}
        {shape && !collapsed && (
          <Tooltip content={shapeLabel} testId={`${testId}-shape-${item.id}`}>
            <span className={styles.rowGlyph} data-shape={shape}>
              {shape === BodyShapeType.MALE ? <MaleIcon /> : <FemaleIcon />}
            </span>
          </Tooltip>
        )}
        <button
          className={styles.dressButton}
          type="button"
          aria-pressed={dressed}
          aria-label={t(dressed ? 'item_editor.sidebar.undress' : 'item_editor.sidebar.dress', { name: item.name })}
          disabled={!available}
          data-testid={`${testId}-dress-${item.id}`}
          onClick={() => onToggleDressed(item)}
        >
          {dressed ? <DressedIcon fontSize="small" /> : <UndressedIcon fontSize="small" />}
        </button>
      </div>
    )
    // Collapsed rows have no visible name or glyph, so the tooltip carries them (and the body-shape note when relevant).
    const note = available ? null : t('item_editor.sidebar.no_representation')
    const content = collapsed ? [item.name, shapeLabel, note].filter(Boolean).join(' · ') : note
    if (content === null) return row
    return (
      <Tooltip
        key={item.id}
        content={content}
        placement={collapsed ? 'right' : 'top'}
        asChild
        testId={available ? `${testId}-name-tooltip` : `${testId}-unavailable`}
      >
        {row}
      </Tooltip>
    )
  }

  function renderGroup(kind: 'wearables' | 'emotes', groupItems: Item[]) {
    return (
      <EditorSection
        title={t(`item_editor.sidebar.${kind}`)}
        icon={kind === 'wearables' ? <WearableIcon /> : <EmoteIcon />}
        iconOnly={collapsed}
        testId={`${testId}-${kind}`}
      >
        <div className={styles.rows}>{groupItems.map(renderRow)}</div>
      </EditorSection>
    )
  }

  function renderReviewGroup(kind: 'to_review' | 'rest', groupItems: Item[]) {
    return (
      <EditorSection
        title={t(`item_editor.sidebar.${kind}`, { count: groupItems.length })}
        icon={kind === 'to_review' ? <ToReviewIcon /> : <RestIcon />}
        iconOnly={collapsed}
        testId={`${testId}-${kind}`}
      >
        <div className={styles.rows}>{groupItems.map(renderRow)}</div>
      </EditorSection>
    )
  }

  const toggleLabel = t(collapsed ? 'item_editor.sidebar.expand' : 'item_editor.sidebar.collapse')

  return (
    <div className={styles.shell}>
      <aside className={styles.wrap} data-testid={testId} data-collapsed={collapsed || undefined}>
        <div className={styles.header}>
          {/* The review bar carries its own back link, so the review sidebar only names the collection. */}
          <div className={styles.headerRow}>
            {mode === 'edit' && back && (
              <Tooltip content={back.label} placement="right" asChild testId={`${testId}-back-tooltip`}>
                <Link
                  className={styles.iconButton}
                  to={back.to}
                  aria-label={back.label}
                  data-testid={`${testId}-back`}
                  // A router link never reaches the unload guard, so leaving is asked about here.
                  onClick={event => {
                    if (onLeave && !onLeave(back.to)) event.preventDefault()
                  }}
                >
                  <BackIcon fontSize="small" />
                </Link>
              </Tooltip>
            )}
            <div className={styles.collectionName}>
              <div className={styles.titleGroup}>
                <h2 className={styles.collectionTitle} title={collection.name} data-testid={`${testId}-collection`}>
                  {collection.name}
                </h2>
                {mode === 'edit' && onRename && (
                  <button
                    className={styles.renameButton}
                    type="button"
                    aria-label={t('collection_detail_page.rename')}
                    data-testid={`${testId}-rename`}
                    onClick={onRename}
                  >
                    <EditIcon />
                  </button>
                )}
              </div>
              {/* The review bar shows the curation state instead. */}
              {mode === 'edit' && collectionStatus && (
                <CollectionStatusPill collection={collection} status={collectionStatus} />
              )}
            </div>
          </div>
          {showAddItems && (
            <div className={styles.headerMeta}>
              {collapsed ? (
                <Tooltip
                  content={t('item_editor.sidebar.add_items')}
                  placement="right"
                  asChild
                  testId={`${testId}-add-items-tooltip`}
                >
                  <Button
                    type="button"
                    variant="secondary"
                    size="icon"
                    aria-label={t('item_editor.sidebar.add_items')}
                    data-testid={`${testId}-add-items`}
                    onClick={onAddItems}
                  >
                    <AddIcon fontSize="small" />
                  </Button>
                </Tooltip>
              ) : (
                <Button
                  type="button"
                  variant="secondary"
                  size="md"
                  data-testid={`${testId}-add-items`}
                  onClick={onAddItems}
                >
                  <AddIcon fontSize="small" />
                  {t('item_editor.sidebar.add_items')}
                </Button>
              )}
            </div>
          )}
        </div>
        <div className={styles.list} ref={listRef}>
          {isLoading ? (
            <div className={styles.rows} aria-busy="true" data-testid={`${testId}-loading`}>
              {Array.from({ length: 5 }, (_, index) => (
                <div className={cx(styles.skeletonRow, 'skeleton')} key={index} />
              ))}
            </div>
          ) : items.length === 0 ? (
            !collapsed && (
              <p className={styles.empty} data-testid={`${testId}-empty`}>
                {t('item_editor.sidebar.empty')}
              </p>
            )
          ) : reviewGroups ? (
            <>
              {renderReviewGroup('to_review', reviewGroups.toReview)}
              {reviewGroups.rest.length > 0 && renderReviewGroup('rest', reviewGroups.rest)}
            </>
          ) : grouped ? (
            <>
              {renderGroup('wearables', groups.wearables)}
              {renderGroup('emotes', groups.emotes)}
            </>
          ) : (
            <div className={styles.rows}>{items.map(renderRow)}</div>
          )}
        </div>
      </aside>
      {/* Half outside the panel edge, level with the back button, so it reads as the panel's own handle. */}
      <Tooltip content={toggleLabel} placement="right" asChild testId={`${testId}-toggle-tooltip`}>
        <button
          className={styles.floatingToggle}
          type="button"
          aria-label={toggleLabel}
          aria-expanded={!collapsed}
          data-testid={`${testId}-toggle`}
          onClick={onToggleCollapsed}
        >
          {collapsed ? <ExpandIcon fontSize="small" /> : <CollapseIcon fontSize="small" />}
        </button>
      </Tooltip>
    </div>
  )
}
