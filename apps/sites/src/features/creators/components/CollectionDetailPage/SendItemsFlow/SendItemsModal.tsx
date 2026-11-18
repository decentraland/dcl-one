import { useMemo } from 'react'
import { AddIcon, ChevronLeftIcon, ChevronRightIcon, ExpandMoreIcon as ExpandIcon } from '~/components/Icons'
import { useTranslation } from '~/intl'
import { useFriends } from '~/hooks/useSales'
import { useProfile } from '~/hooks/useProfile'
import { type Session } from '~/lib/auth'
import { getContentsStorageUrl } from '~/lib/builder'
import { type Friend } from '~/lib/friends'
import { type Item } from '~/lib/items'
import {
  MAX_ITEMS_PER_SEND,
  copiesPerRecipient,
  createTransfer,
  getStock,
  totalCopies,
  type RecipientBundle,
  type Transfer,
  type TransferDraft
} from '~/lib/mint'
import { Button } from '~/components/Button'
import { ItemThumbnail } from '~/components/ItemThumbnail'
import { Modal } from '~/components/Modal'
import { StepIndicator } from '~/components/StepIndicator'
import sell from '../SellItemFlow/SellItemModal.module.css'
import styles from './SendItemsModal.module.css'
import { TransferCard } from './TransferCard'

export type SendStep = 'select' | 'confirm'

type Props = {
  collectionId: string
  items: Item[]
  session: Session
  step: SendStep
  transfers: TransferDraft[]
  onStep: (step: SendStep) => void
  onTransfers: (transfers: TransferDraft[]) => void
  /** A submit is in flight (custodial wallet, no prompt to wait for): the button spins. */
  busy: boolean
  onSubmit: () => void
  onClose: () => void
}

/** Two steps: pick recipients and copies per transfer, then review what each wallet ends up with before sending. */
export function SendItemsModal({
  collectionId,
  items,
  session,
  step,
  transfers,
  onStep,
  onTransfers,
  busy,
  onSubmit,
  onClose
}: Props) {
  const { t } = useTranslation()
  const friends = useFriends(session, true)
  const total = useMemo(() => totalCopies(transfers), [transfers])
  const over = total > MAX_ITEMS_PER_SEND
  // Every recipient gets at least one copy: at the cap, another transfer could send nothing.
  const recipientsFull = useMemo(
    () => transfers.reduce((sum, transfer) => sum + transfer.recipients.length, 0) >= MAX_ITEMS_PER_SEND,
    [transfers]
  )
  const bundles = useMemo(() => copiesPerRecipient(transfers), [transfers])
  const itemCount = useMemo(() => new Set(bundles.flatMap(bundle => Object.keys(bundle.amounts))).size, [bundles])

  function replace(index: number, transfer: Transfer) {
    onTransfers(transfers.map((current, i) => (i === index ? { ...transfer, key: current.key } : current)))
  }

  return (
    <Modal title={t('send_items_modal.title')} compact onClose={onClose} closeDisabled={busy} testId="send-items-modal">
      <hr className={sell.divider} />
      <div className={styles.steps}>
        <StepIndicator
          current={step === 'select' ? 1 : 2}
          total={2}
          labels={[t('send_items_modal.step_select'), t('send_items_modal.step_confirm')]}
          testId="send-steps"
        />
      </div>
      {/* Keyed by step so the body remounts and the confirm step opens at the top, not where the list was scrolled. */}
      <div
        className={styles.body}
        key={step}
        data-busy={busy || undefined}
        aria-busy={busy || undefined}
        {...(busy ? { inert: '' } : {})}
      >
        {step === 'select' ? (
          <>
            {transfers.map((transfer, index) => (
              <TransferCard
                key={transfer.key}
                collectionId={collectionId}
                index={index}
                transfers={transfers}
                items={items}
                friends={friends.data}
                isLoadingFriends={friends.isLoading}
                self={session.address.toLowerCase()}
                onChange={transfer => replace(index, transfer)}
                onRemove={transfers.length > 1 ? () => onTransfers(transfers.filter((_, i) => i !== index)) : undefined}
              />
            ))}
            {!recipientsFull && (
              <Button
                className={styles.addTransfer}
                type="button"
                variant="secondary"
                data-testid="send-add-transfer"
                onClick={() => onTransfers([...transfers, createTransfer()])}
              >
                <AddIcon fontSize="small" />
                {t('send_items_modal.add_transfer')}
              </Button>
            )}
            <p className={styles.total} data-over={over || undefined} data-testid="send-total">
              <span>{t('send_items_modal.total', { count: total, max: MAX_ITEMS_PER_SEND })}</span>
              {over && <span>{t('send_items_modal.limit_reached', { max: MAX_ITEMS_PER_SEND })}</span>}
            </p>
          </>
        ) : (
          <>
            <p className={styles.intro}>{t('send_items_modal.confirm_intro', { count: total })}</p>
            <details className={styles.summary} open data-testid="send-summary">
              <summary>
                {t('send_items_modal.summary', { recipients: bundles.length, items: itemCount, copies: total })}
                <ExpandIcon />
              </summary>
              <div className={styles.summaryBody}>
                {bundles.map(bundle => (
                  <RecipientSummary key={bundle.address} bundle={bundle} items={items} friends={friends.data} />
                ))}
              </div>
            </details>
          </>
        )}
      </div>

      <div className={sell.footer}>
        {step === 'select' ? (
          <Button type="button" variant="secondary" data-testid="send-cancel" onClick={onClose}>
            {t('sell_item_modal.cancel')}
          </Button>
        ) : (
          <Button
            type="button"
            variant="secondary"
            disabled={busy}
            data-testid="send-back"
            onClick={() => onStep('select')}
          >
            <ChevronLeftIcon fontSize="small" />
            {t('send_items_modal.back')}
          </Button>
        )}
        {step === 'select' ? (
          <Button
            type="button"
            variant="primary"
            disabled={total === 0 || over}
            data-testid="send-continue"
            onClick={() => onStep('confirm')}
          >
            {t('send_items_modal.continue')}
            <ChevronRightIcon fontSize="small" />
          </Button>
        ) : (
          <Button type="button" variant="primary" loading={busy} data-testid="send-submit" onClick={onSubmit}>
            {t('send_items_modal.submit')}
          </Button>
        )}
      </div>
    </Modal>
  )
}

type SummaryProps = { bundle: RecipientBundle; items: Item[]; friends: Friend[] | undefined }

function RecipientSummary({ bundle, items, friends }: SummaryProps) {
  const { t } = useTranslation()
  const chosen = useMemo(() => items.filter(item => (bundle.amounts[item.id] ?? 0) > 0), [items, bundle])
  return (
    <div className={styles.recipientCard} data-testid={`send-summary-${bundle.address}`}>
      <RecipientLine address={bundle.address} friends={friends} />
      <ul className={styles.items}>
        {chosen.map(item => {
          const thumbnailHash = item.contents[item.thumbnail]
          const stock = getStock(item)
          return (
            <li className={styles.itemRow} key={item.id}>
              <div className={styles.thumb}>
                <ItemThumbnail src={thumbnailHash ? getContentsStorageUrl(thumbnailHash) : null} rarity={item.rarity} />
              </div>
              <div className={styles.itemText}>
                <span className={styles.itemName} title={item.name}>
                  {item.name}
                </span>
                <span className={styles.label}>
                  {t('send_items_modal.available', { available: stock.available, total: stock.total })}
                </span>
              </div>
              <span className={styles.amount} data-testid={`send-summary-${bundle.address}-${item.id}`}>
                {bundle.amounts[item.id]}
              </span>
            </li>
          )
        })}
      </ul>
    </div>
  )
}

type RecipientLineProps = { address: string; friends: Friend[] | undefined; testId?: string }

/** One wallet as a person: avatar and name when known (a friend or a claimed profile), the address always. */
export function RecipientLine({ address, friends, testId = 'send-summary-recipient' }: RecipientLineProps) {
  const friend = useMemo(() => friends?.find(candidate => candidate.address === address), [friends, address])
  const profile = useProfile(friend ? undefined : address)
  const name = friend?.name ?? profile.data?.name
  const avatar = friend?.avatarUrl ?? profile.data?.avatar?.snapshots?.face256
  return (
    <div className={styles.recipientLine} data-testid={testId}>
      {avatar ? (
        <img className={sell.avatar} src={avatar} alt="" />
      ) : (
        <span className={sell.avatarFallback} aria-hidden />
      )}
      {name && <b>{name}</b>}
      <span title={address}>{name ? `(${address})` : address}</span>
    </div>
  )
}
