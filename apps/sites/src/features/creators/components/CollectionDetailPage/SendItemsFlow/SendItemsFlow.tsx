import { useMemo, useRef, useState } from 'react'
import { useTranslation } from '@/data/lib/creators/intl'
import sentArt from '@/features/creators/assets/send-success.png'
import { useDiscardGuard } from '@/features/creators/hooks/useDiscardGuard'
import { useSendItems } from '@/features/creators/hooks/useSales'
import { isSocialLogin, type Session } from '@/data/lib/creators/auth'
import { type Collection } from '@/data/lib/creators/collections'
import { type Item } from '@/data/lib/creators/items'
import { createTransfer, hasSendDraft, isSendableItem, type TransferDraft } from '@/data/lib/creators/mint'
import { toSellItemError, type SellFailureReason } from '@/data/lib/creators/sales'
import { ConfirmModal } from '@/features/creators/components/ConfirmModal'
import { PendingModal } from '@/features/creators/components/PendingModal'
import { SaleErrorModal } from '../SellItemFlow/SaleErrorModal'
import { SuccessModal } from '@/features/creators/components/SuccessModal'
import { SendItemsModal, type SendStep } from './SendItemsModal'

type View = 'form' | 'sending' | 'success' | 'error'
type Phase = 'confirm' | 'pending'

type Props = {
  collection: Collection
  items: Item[]
  session: Session
  onClose: () => void
}

/**
 * Send copies of the collection's items straight to wallets. Same shape as SellItemFlow: web3 wallets get
 * the whole-dialog "confirm in your wallet" status, custodial ones just see the submit button spin; a
 * failure's TRY AGAIN reopens the confirm step with everything kept. Closing with a recipient or a copy
 * entered asks first.
 */
export function SendItemsFlow({ collection, items, session, onClose }: Props) {
  const { t } = useTranslation()
  const social = isSocialLogin(session)
  const sendable = useMemo(() => items.filter(isSendableItem), [items])

  const [view, setView] = useState<View>('form')
  const [phase, setPhase] = useState<Phase>('confirm')
  const [step, setStep] = useState<SendStep>('select')
  const [transfers, setTransfers] = useState<TransferDraft[]>(() => [createTransfer()])
  const [reason, setReason] = useState<SellFailureReason | null>(null)
  // Bumped when the creator backs out of a wallet prompt, so that attempt's outcome is ignored.
  const attempt = useRef(0)

  const send = useSendItems(session)
  const dirty = useMemo(() => hasSendDraft(transfers), [transfers])
  const busy = social && send.isPending
  const { isDiscardOpen, requestClose, keepEditing } = useDiscardGuard({
    state:
      view === 'form'
        ? busy
          ? 'busy'
          : 'editing'
        : view === 'sending'
          ? 'busy'
          : view === 'error'
            ? 'failed'
            : 'done',
    dirty,
    pending: send.isPending,
    onClose
  })

  function submit() {
    const id = ++attempt.current
    setPhase('confirm')
    if (!social) setView('sending')
    send.mutate(
      { collection, items: sendable, transfers, onSigned: () => attempt.current === id && setPhase('pending') },
      {
        onSuccess: () => attempt.current === id && setView('success'),
        onError: cause => {
          if (attempt.current !== id) return
          const failure = toSellItemError(cause).reason
          // Dismissing the wallet prompt is the creator changing their mind, not a failure to report.
          if (failure === 'rejected') return setView('form')
          setReason(failure)
          setView('error')
        }
      }
    )
  }

  switch (view) {
    case 'form':
      return (
        <>
          <SendItemsModal
            collectionId={collection.id}
            items={sendable}
            session={session}
            step={step}
            transfers={transfers}
            onStep={setStep}
            onTransfers={setTransfers}
            busy={busy}
            onSubmit={submit}
            onClose={requestClose}
          />
          {isDiscardOpen && (
            <ConfirmModal
              title={t('send_items_modal.discard.title')}
              description={t('send_items_modal.discard.description')}
              onClose={keepEditing}
              cancel={{ label: t('send_items_modal.discard.leave'), onClick: onClose, testId: 'discard-send-leave' }}
              confirm={{ label: t('send_items_modal.discard.keep'), onClick: keepEditing, testId: 'discard-send-keep' }}
              testId="discard-send-modal"
            />
          )}
        </>
      )
    case 'sending':
      return (
        <PendingModal
          label={phase === 'confirm' ? t('sell_item_modal.confirm_in_wallet') : t('send_items_modal.sending')}
          onCancel={
            phase === 'confirm'
              ? () => {
                  attempt.current++
                  setView('form')
                }
              : undefined
          }
          testId="send-items-pending"
        />
      )
    case 'success':
      return (
        <SuccessModal
          title={t('send_items_modal.success_title')}
          description={t('send_items_modal.success_description')}
          art={sentArt}
          onDone={onClose}
        />
      )
    case 'error':
      return reason ? (
        <SaleErrorModal
          stage="send"
          reason={reason}
          onCancel={onClose}
          onRetry={() => {
            setReason(null)
            setView('form')
          }}
        />
      ) : null
  }
}
