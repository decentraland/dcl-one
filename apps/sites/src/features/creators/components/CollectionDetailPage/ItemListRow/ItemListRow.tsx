import { useState, type KeyboardEvent, type ReactNode } from 'react'
import { useIntl } from 'react-intl'
import { useTranslation } from '~/intl'
import { getContentsStorageUrl } from '~/lib/builder'
import { ITEM_NAME_MAX_LENGTH, isValidItemName } from '~/lib/itemFactory'
import { ItemType, getItemBodyShapeType, getItemSales, isSmartWearable, type Item } from '~/lib/items'
import { EmotePlayMode } from '~/lib/itemFactory'
import { type ItemListing } from '~/lib/listings'
import { type ItemRowStatus } from '~/lib/itemSync'
import { type ItemValidation } from '~/hooks/useCollectionValidation'
import { discountedCreditsPpm, discountedManaWeiPpm } from '~/lib/collectionDiscount'
import { formatCredits, formatMana } from '~/lib/publishFee'
import { shopItemUrl } from '~/lib/shop'
import { CurrencyAmount } from '~/components/CurrencyAmount'
import {
  SmartIcon,
  CheckIcon,
  CloseIcon,
  EditIcon,
  ErrorOutlineIcon as WarningIcon,
  ReportProblemOutlinedIcon as ErrorIcon
} from '~/components/Icons'
import { Tooltip } from '~/components/Tooltip'
import { BodyShapeIcon, CategoryIcon, PlayModeIcon } from '~/components/ItemIcons'
import { ItemThumbnail } from '~/components/ItemThumbnail'
import { RarityPill } from '~/components/RarityPill'
import { ItemSaleStatus } from '../ItemSaleStatus'
import { ItemSyncPill } from '../ItemSyncPill'
import { SaleTag } from '../SaleTag'
import { cx } from '~/lib/cx'
import styles from './ItemListRow.module.css'
import shared from '~/styles/shared.module.css'

const EMPTY = '—'

type Props = {
  item: Item
  /** Lay out the Play Mode column; the list shows it only when the current view has emotes. */
  withPlayMode?: boolean
  /** Lay out the Price, Sales and Sale Status columns; the list shows them once the collection has been published. */
  withMarket?: boolean
  /** The item's primary listing: `null` when it has none, `undefined` while listings are still loading. */
  listing?: ItemListing | null
  /** Parts per million a running collection discount takes off this listing in the Shop, when one does. */
  discountPpm?: number
  /** Lay out the Status column; the list shows it only when some row on the page is not plain published. */
  withStatus?: boolean
  /** The row's Status pill; `null` while the sync is still unknown. */
  syncStatus?: ItemRowStatus | null
  /** Whether the collection has been approved at least once, so its items can be put on sale. */
  canSell?: boolean
  onPutOnSale?: (item: Item) => void
  /** Offered on the price when the viewer may re-price the listing. */
  onEditPrice?: (item: Item) => void
  /** Whether the viewer may edit the name and thumbnail in place. */
  editable?: boolean
  /** Saves the new name; a rejected promise keeps the row in edit mode. */
  onRename?: (item: Item, name: string) => Promise<unknown>
  onEditThumbnail?: (item: Item) => void
  /** The collection's contract, for the Shop link on a listed item. */
  contractAddress?: string
  /** The row's ⋯ menu; the page supplies it once the viewer is signed in. */
  actions?: ReactNode
  /** The item's checks; only errors and warnings show. */
  validation?: ItemValidation
  /** Whether errors block publishing, which changes the indicator's copy. */
  validationBlocks?: boolean
  onShowValidation?: (item: Item) => void
}

export function ItemListRow({
  item,
  withPlayMode = false,
  withMarket = false,
  listing,
  discountPpm,
  withStatus = false,
  syncStatus = null,
  canSell = false,
  onPutOnSale,
  onEditPrice,
  editable = false,
  onRename,
  onEditThumbnail,
  contractAddress,
  actions,
  validation,
  validationBlocks = false,
  onShowValidation
}: Props) {
  const { t } = useTranslation()
  const intl = useIntl()
  // The name being typed; `null` while the name is not being edited.
  const [draftName, setDraftName] = useState<string | null>(null)
  const [isSaving, setSaving] = useState(false)

  const thumbnailHash = item.contents[item.thumbnail]
  const bodyShapeType = getItemBodyShapeType(item)
  const category = item.data.category
  const isEmote = item.type === ItemType.EMOTE
  const isSmart = isSmartWearable(item)
  const sales = withMarket ? getItemSales(item) : undefined
  const isSoldOut = !!sales && sales.minted >= sales.maxSupply
  // Blank while listings load, and for a viewer who may not sell an unlisted item (the sold-out pill still shows).
  const isSaleStatusEmpty = listing === undefined || (listing === null && !onPutOnSale && !isSoldOut)
  const shopUrl = withMarket && contractAddress && item.tokenId ? shopItemUrl(contractAddress, item.tokenId) : undefined
  const canRename = editable && !!onRename
  const canEditThumbnail = editable && !!onEditThumbnail
  const isEditingName = draftName !== null
  const draftValid = draftName !== null && isValidItemName(draftName)
  const flagged = validation?.status === 'errors' || validation?.status === 'warnings' ? validation : undefined
  const validationCopy = !flagged
    ? null
    : flagged.status === 'errors'
      ? t(`item_validation.row.${validationBlocks ? 'errors_blocking' : 'errors'}`)
      : t('item_validation.row.warnings')

  function startEditing() {
    setDraftName(item.name)
  }

  function stopEditing() {
    if (!isSaving) setDraftName(null)
  }

  async function saveName() {
    if (draftName === null || !onRename || isSaving) return
    const name = draftName.trim()
    if (name === item.name) {
      stopEditing()
      return
    }
    if (!isValidItemName(name)) return
    setSaving(true)
    try {
      await onRename(item, name)
      stopEditing()
    } catch {
      // The page reports the failure; the row keeps the draft so the user can retry.
    } finally {
      setSaving(false)
    }
  }

  function onNameKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === 'Enter') {
      event.preventDefault()
      void saveName()
    } else if (event.key === 'Escape') {
      event.preventDefault()
      stopEditing()
    }
  }

  function renderAmount(listing: ItemListing) {
    const isFree = listing.currency === 'mana' ? listing.manaWei === 0n : listing.credits === 0
    if (isFree) return t('collection_detail_page.price.free')
    const currency = listing.currency
    const amount = listing.currency === 'mana' ? formatMana(listing.manaWei) : formatCredits(listing.credits)
    const title =
      currency === 'mana'
        ? t('collection_detail_page.price.mana', { amount })
        : t('collection_detail_page.price.credits', { amount })
    const sale = !discountPpm
      ? amount
      : listing.currency === 'mana'
        ? formatMana(discountedManaWeiPpm(listing.manaWei, discountPpm))
        : formatCredits(discountedCreditsPpm(listing.credits, discountPpm))
    // A cheap item can round a discount away; then there is nothing to show but its price.
    if (sale === amount) {
      return (
        <span className={styles.amount} title={title}>
          <CurrencyAmount currency={currency}>{amount}</CurrencyAmount>
        </span>
      )
    }
    const discountPct = discountPpm! / 10_000
    return (
      <span
        className={styles.amount}
        title={t('collection_detail_page.price.discounted', {
          amount: sale,
          original: amount,
          pct: intl.formatNumber(discountPct, { maximumFractionDigits: 2 })
        })}
        data-discounted
        data-testid="item-row-discounted-price"
      >
        <SaleTag pct={discountPct} />
        <span className={styles.salePrice}>
          <CurrencyAmount currency={currency}>{sale}</CurrencyAmount>
        </span>
        <s className={styles.originalPrice}>{amount}</s>
      </span>
    )
  }

  function renderPrice() {
    if (listing === undefined) return null
    if (listing === null) return EMPTY
    const amount = renderAmount(listing)
    if (!onEditPrice) return amount
    return (
      <button
        className={styles.priceButton}
        type="button"
        title={t('collection_detail_page.item_actions.edit_price')}
        aria-label={t('collection_detail_page.item_actions.edit_price')}
        data-testid="item-row-edit-price"
        onClick={() => onEditPrice(item)}
      >
        {amount}
        <EditIcon aria-hidden />
      </button>
    )
  }

  const thumbnail = (
    <ItemThumbnail src={thumbnailHash ? getContentsStorageUrl(thumbnailHash) : null} rarity={item.rarity} />
  )

  const validationChip = flagged && validationCopy && (
    <Tooltip content={validationCopy} asChild testId="item-row-validation-tooltip">
      <button
        className={styles.validationChip}
        type="button"
        aria-label={validationCopy}
        data-status={flagged.status}
        data-testid="item-row-validation"
        onClick={() => onShowValidation?.(item)}
      >
        {flagged.status === 'errors' ? <ErrorIcon /> : <WarningIcon />}
      </button>
    </Tooltip>
  )

  return (
    <article
      className={cx(styles.columns, styles.row)}
      data-testid="item-row"
      data-with-play-mode={withPlayMode || undefined}
      data-with-market={withMarket || undefined}
      data-with-status={withStatus || undefined}
      data-validation={flagged?.status}
    >
      <div className={styles.thumb}>
        {canEditThumbnail ? (
          <button
            className={styles.thumbButton}
            type="button"
            title={t('collection_detail_page.item_row.edit_thumbnail')}
            aria-label={t('collection_detail_page.item_row.edit_thumbnail')}
            data-testid="item-row-edit-thumbnail"
            onClick={() => onEditThumbnail?.(item)}
          >
            {thumbnail}
            <span className={styles.thumbBadge} aria-hidden>
              <EditIcon />
            </span>
          </button>
        ) : (
          thumbnail
        )}
      </div>
      <div className={styles.content}>
        {isEditingName ? (
          <div className={styles.nameEditor} data-testid="item-row-name-editor">
            <input
              className={styles.nameInput}
              value={draftName}
              maxLength={ITEM_NAME_MAX_LENGTH}
              disabled={isSaving}
              autoFocus
              aria-label={t('collection_detail_page.item_row.name_placeholder')}
              placeholder={t('collection_detail_page.item_row.name_placeholder')}
              data-invalid={draftName && !draftValid ? true : undefined}
              data-testid="item-row-name-input"
              onChange={event => setDraftName(event.target.value)}
              onKeyDown={onNameKeyDown}
            />
            <button
              className={styles.editorButton}
              type="button"
              data-variant="cancel"
              title={t('collection_detail_page.item_row.cancel_edit')}
              aria-label={t('collection_detail_page.item_row.cancel_edit')}
              disabled={isSaving}
              data-testid="item-row-name-cancel"
              onClick={stopEditing}
            >
              <CloseIcon />
            </button>
            <button
              className={styles.editorButton}
              type="button"
              data-variant="save"
              title={t('collection_detail_page.item_row.save')}
              aria-label={t('collection_detail_page.item_row.save')}
              disabled={!draftValid || isSaving}
              aria-busy={isSaving || undefined}
              data-testid="item-row-name-save"
              onClick={() => void saveName()}
            >
              {isSaving ? <span className={shared.spinner} aria-hidden /> : <CheckIcon />}
            </button>
          </div>
        ) : (
          <span className={styles.name}>
            {canRename ? (
              <button
                className={styles.nameButton}
                type="button"
                title={t('collection_detail_page.item_row.edit_name')}
                data-testid="item-row-edit-name"
                onClick={startEditing}
              >
                <span className={styles.nameText} data-testid="item-row-name">
                  {item.name}
                </span>
                <EditIcon aria-hidden />
              </button>
            ) : (
              <span className={styles.nameText} data-testid="item-row-name" title={item.name}>
                {item.name}
              </span>
            )}
            {isSmart && (
              <Tooltip content={t('collection_detail_page.smart_wearable')} asChild testId="item-row-smart-tooltip">
                <span className={styles.smartBadge} data-testid="item-row-smart" tabIndex={0}>
                  <SmartIcon />
                </span>
              </Tooltip>
            )}
            {validationChip}
          </span>
        )}
        <div className={styles.cell} data-testid="item-row-body-shape">
          {bodyShapeType ? <BodyShapeIcon bodyShape={bodyShapeType} withLabel /> : EMPTY}
        </div>
        <div className={styles.cell} data-testid="item-row-category">
          {category ? <CategoryIcon category={category} withLabel /> : EMPTY}
        </div>
        {withPlayMode && (
          <div className={styles.cell} data-testid="item-row-play-mode" data-empty={!isEmote || undefined}>
            {isEmote ? (
              <PlayModeIcon playMode={item.data.loop ? EmotePlayMode.LOOP : EmotePlayMode.SIMPLE} withLabel />
            ) : (
              EMPTY
            )}
          </div>
        )}
        <div className={styles.cell} data-testid="item-row-rarity">
          {item.rarity && <RarityPill rarity={item.rarity} />}
        </div>
        {withMarket && (
          <>
            <div
              className={styles.cell}
              data-testid="item-row-price"
              data-currency={listing?.currency}
              data-empty={listing === null || undefined}
            >
              {renderPrice()}
            </div>
            <div className={styles.cell} data-testid="item-row-sales" data-empty={!sales || undefined}>
              {sales ? `${intl.formatNumber(sales.minted)}/${intl.formatNumber(sales.maxSupply)}` : EMPTY}
            </div>
            <div className={styles.cell} data-testid="item-row-sale-status" data-empty={isSaleStatusEmpty || undefined}>
              <ItemSaleStatus
                sales={sales}
                listing={listing}
                canSell={canSell}
                onPutOnSale={onPutOnSale && (() => onPutOnSale(item))}
                shopUrl={shopUrl}
              />
            </div>
          </>
        )}
        {withStatus && (
          <div className={styles.cell} data-testid="item-row-status" data-empty={!syncStatus || undefined}>
            {syncStatus && <ItemSyncPill status={syncStatus} />}
          </div>
        )}
      </div>
      <div className={styles.actionsCell}>{actions}</div>
    </article>
  )
}
