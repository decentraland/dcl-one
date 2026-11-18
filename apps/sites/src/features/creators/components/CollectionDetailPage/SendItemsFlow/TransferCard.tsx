import { useState } from 'react'
import { AddIcon, CloseIcon, RemoveIcon } from '../../Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { track } from '@/data/lib/creators/analytics'
import { getContentsStorageUrl } from '@/data/lib/creators/builder'
import type { Friend } from '@/data/lib/creators/friends'
import type { Item } from '@/data/lib/creators/items'
import {
  MAX_ITEMS_PER_SEND,
  allocatedElsewhere,
  getStock,
  maxAmount,
  parseRecipientList,
  type Transfer
} from '@/data/lib/creators/mint'
import { useNotifications } from '@/data/lib/creators/notifications'
import { Checkbox } from '../../Checkbox'
import { ItemThumbnail } from '../../ItemThumbnail'
import { BeneficiaryInput } from '../SellItemFlow/BeneficiaryInput'
import styles from './SendItemsModal.module.css'

type Props = {
  collectionId: string
  index: number
  transfers: Transfer[]
  items: Item[]
  friends: Friend[] | undefined
  isLoadingFriends: boolean
  /** The signed-in wallet, offered first in the recipient list. */
  self: string
  onChange: (transfer: Transfer) => void
  onRemove?: () => void
}

/** One transfer: its recipients, and how many copies of each item every one of them gets. */
export function TransferCard({
  collectionId,
  index,
  transfers,
  items,
  friends,
  isLoadingFriends,
  self,
  onChange,
  onRemove
}: Props) {
  const { t } = useTranslation()
  const showToast = useNotifications(state => state.showToast)
  const transfer = transfers[index]
  const [adding, setAdding] = useState(false)
  // Every recipient gets at least one copy, so a transfer at the send cap takes no more.
  const full = transfer.recipients.length >= MAX_ITEMS_PER_SEND
  const showInput = !full && (transfer.recipients.length === 0 || adding)
  const testId = `transfer-${index + 1}`

  function setRecipients(recipients: string[]) {
    // More recipients may leave the amounts over the stock: bring them back within it.
    const scaled = { ...transfers[index], recipients }
    const next = transfers.map((current, i) => (i === index ? scaled : current))
    const amounts = Object.fromEntries(
      items
        .map(item => [item.id, Math.min(scaled.amounts[item.id] ?? 0, maxAmount(item, next, index))] as const)
        .filter(([, amount]) => amount > 0)
    )
    onChange({ recipients, amounts })
  }

  /** A pasted list: every new address joins the transfer, and a toast says how many did (or why none could). */
  function addRecipients(text: string): string | null {
    const { added, invalid, repeated, cut } = parseRecipientList(text, transfer.recipients)
    track('Paste recipients', { collectionId, added: added.length, invalid, repeated, cut })
    if (added.length === 0) {
      if (invalid > 0) return t('sell_item_modal.beneficiary.invalid')
      // Only repeats, dropped like a typed one: the field clears and a toast says why.
      showToast(t('send_items_modal.duplicate_recipient'), { type: 'info' })
      return null
    }
    setRecipients([...transfer.recipients, ...added])
    setAdding(false)
    const count = added.length
    const skipped = invalid + repeated
    if (cut > 0) showToast(t('send_items_modal.paste_cut', { count, max: MAX_ITEMS_PER_SEND }), { type: 'warn' })
    else if (skipped > 0) showToast(t('send_items_modal.paste_skipped', { count, skipped }), { type: 'warn' })
    else showToast(t('send_items_modal.paste_added', { count }))
    return null
  }

  function setAmount(itemId: string, amount: number) {
    const amounts = { ...transfer.amounts }
    if (amount > 0) amounts[itemId] = amount
    else delete amounts[itemId]
    onChange({ ...transfer, amounts })
  }

  return (
    <section className={styles.transfer} data-testid={testId}>
      <div className={styles.transferHead}>
        {t('send_items_modal.transfer', { n: index + 1 })}
        {onRemove && (
          <button
            className={styles.removeTransfer}
            type="button"
            aria-label={t('send_items_modal.remove_transfer')}
            data-testid={`${testId}-remove`}
            onClick={onRemove}
          >
            <CloseIcon fontSize="small" />
          </button>
        )}
      </div>

      <ol className={styles.recipients}>
        {transfer.recipients.map((address, i) => (
          <li className={styles.recipient} key={address}>
            <span className={styles.index}>{i + 1}</span>
            <BeneficiaryInput
              value={address}
              onChange={() => setRecipients(transfer.recipients.filter(other => other !== address))}
              friends={friends}
              isLoadingFriends={isLoadingFriends}
              self={self}
              variant="compact"
              testId={`${testId}-recipient`}
            />
          </li>
        ))}
        {showInput && (
          <li className={styles.recipient}>
            <span className={styles.index}>{transfer.recipients.length + 1}</span>
            <BeneficiaryInput
              value=""
              onChange={address => {
                setRecipients([...transfer.recipients, address])
                setAdding(false)
              }}
              friends={friends}
              isLoadingFriends={isLoadingFriends}
              placeholder={t('send_items_modal.recipient_placeholder')}
              self={self}
              exclude={transfer.recipients}
              onExcluded={() => showToast(t('send_items_modal.duplicate_recipient'), { type: 'info' })}
              onChangeMany={addRecipients}
              testId={`${testId}-recipient`}
            />
          </li>
        )}
      </ol>
      <div className={styles.recipientActions}>
        {!full && (
          <button
            className={styles.linkButton}
            type="button"
            disabled={showInput}
            data-testid={`${testId}-add-recipient`}
            onClick={() => setAdding(true)}
          >
            <AddIcon />
            {t('send_items_modal.add_recipient')}
          </button>
        )}
        {transfer.recipients.length > 1 && (
          <button
            className={styles.linkButton}
            type="button"
            data-testid={`${testId}-clear-recipients`}
            onClick={() => {
              setRecipients([])
              setAdding(false)
            }}
          >
            <CloseIcon />
            {t('send_items_modal.clear_recipients')}
          </button>
        )}
      </div>

      {transfer.recipients.length > 0 && (
        <span className={styles.label}>{t('send_items_modal.items_for_recipient')}</span>
      )}
      <ul className={styles.items}>
        {items.map(item => {
          const amount = transfer.amounts[item.id] ?? 0
          const stock = getStock(item)
          const max = maxAmount(item, transfers, index)
          const soldOut = stock.available === 0
          const remaining = stock.available - allocatedElsewhere(transfers, item.id)
          const thumbnailHash = item.contents[item.thumbnail]
          return (
            <li
              className={styles.itemRow}
              key={item.id}
              data-disabled={soldOut || undefined}
              data-testid={`${testId}-item-${item.id}`}
            >
              <Checkbox
                checked={amount > 0}
                disabled={soldOut || (amount === 0 && max === 0)}
                onChange={checked => setAmount(item.id, checked ? 1 : 0)}
                testId={`${testId}-item-${item.id}-check`}
              >
                <span className={styles.srOnly}>{t('send_items_modal.select_item', { name: item.name })}</span>
              </Checkbox>
              <div className={styles.thumb}>
                <ItemThumbnail
                  src={thumbnailHash ? getContentsStorageUrl(thumbnailHash) : null}
                  rarity={item.rarity}
                  testId={`${testId}-item-${item.id}-thumb`}
                />
              </div>
              <div className={styles.itemText}>
                <span className={styles.itemName} title={item.name}>
                  {item.name}
                </span>
                <span className={styles.label} data-testid={`${testId}-item-${item.id}-stock`}>
                  {soldOut
                    ? t('send_items_modal.sold_out')
                    : t('send_items_modal.available', { available: remaining, total: stock.total })}
                </span>
              </div>
              <div className={styles.stepper}>
                <button
                  type="button"
                  aria-label={t('send_items_modal.decrease', { name: item.name })}
                  disabled={amount === 0}
                  data-testid={`${testId}-item-${item.id}-minus`}
                  onClick={() => setAmount(item.id, amount - 1)}
                >
                  <RemoveIcon />
                </button>
                <AmountInput
                  amount={amount}
                  max={max}
                  disabled={soldOut || (amount === 0 && max === 0)}
                  label={t('send_items_modal.copies_of', { name: item.name })}
                  testId={`${testId}-item-${item.id}-amount`}
                  onChange={next => setAmount(item.id, next)}
                />
                <button
                  type="button"
                  aria-label={t('send_items_modal.increase', { name: item.name })}
                  disabled={soldOut || amount >= max}
                  data-testid={`${testId}-item-${item.id}-plus`}
                  onClick={() => setAmount(item.id, amount + 1)}
                >
                  <AddIcon />
                </button>
              </div>
            </li>
          )
        })}
      </ul>
    </section>
  )
}

type AmountProps = {
  amount: number
  max: number
  disabled: boolean
  label: string
  testId: string
  onChange: (amount: number) => void
}

/** The stepper's count, typed directly: digits only, clamped to the cap; emptied and left, it drops to 0. */
function AmountInput({ amount, max, disabled, label, testId, onChange }: AmountProps) {
  // Stays empty while cleared mid-edit, so the creator can retype without fighting the clamp.
  const [cleared, setCleared] = useState(false)

  function handleChange(value: string) {
    const digits = value.replace(/\D/g, '')
    setCleared(digits === '')
    if (digits !== '') onChange(Math.min(Number(digits), max))
  }

  return (
    <input
      type="text"
      inputMode="numeric"
      autoComplete="off"
      aria-label={label}
      disabled={disabled}
      value={cleared ? '' : String(amount)}
      data-testid={testId}
      onChange={event => handleChange(event.target.value)}
      onFocus={event => event.target.select()}
      onBlur={() => {
        if (cleared) onChange(0)
        setCleared(false)
      }}
    />
  )
}
