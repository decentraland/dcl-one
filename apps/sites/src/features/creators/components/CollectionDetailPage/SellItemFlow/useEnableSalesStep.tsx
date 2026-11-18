import { useRef, useState, type ReactElement } from 'react'
import { useTranslation } from '~/intl'
import { useEnableSales, useSalesEnabled } from '~/hooks/useSales'
import { isSocialLogin, type Session } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { toSellItemError, type SellFailureReason } from '~/lib/sales'
import { PendingModal } from '~/components/PendingModal'
import { EnableSalesModal, type EnableSalesReason } from './EnableSalesModal'

// A wallet transaction/signature is first prompted, then (once signed) confirmed or stored.
export type Phase = 'confirm' | 'pending'

type Step = 'enable' | 'enabling' | 'done'

type Options = {
  collection: Collection
  session: Session
  reason: EnableSalesReason
  onClose: () => void
  /** The transaction failed for a reason other than the creator dismissing the wallet prompt. */
  onFailed: (reason: SellFailureReason) => void
}

type EnableSalesStep = {
  /**
   * The Enable Sales dialog or its wallet status while sales still need enabling; null once they are. After a
   * failure it is the dialog again, so the flow's error view retries by stepping aside.
   */
  modal: ReactElement | null
  isPending: boolean
}

/**
 * The Enable Sales step a sale starts with while the current marketplace cannot mint the collection: one
 * transaction, with a whole-dialog "confirm in your wallet" status for web3 wallets that the creator can
 * back out of. A backed-out transaction can still be signed, so it is never sent twice: confirming again
 * picks the one in flight back up, or moves on if it already went through. Backing out of that resumed prompt
 * while it is still unsigned gives up on it, so a wallet that never settles cannot block sending a new one.
 */
export function useEnableSalesStep({ collection, session, reason, onClose, onFailed }: Options): EnableSalesStep {
  const { t } = useTranslation()
  const social = isSocialLogin(session)
  const salesEnabled = useSalesEnabled(collection)
  const enableSales = useEnableSales(session)

  const [step, setStep] = useState<Step>(salesEnabled ? 'done' : 'enable')
  const [phase, setPhase] = useState<Phase>('confirm')
  // Bumped when the creator backs out of the wallet prompt, so that attempt's outcome is ignored.
  const attempt = useRef(0)
  // The unsettled transaction, if any, and whether its wallet prompt was signed.
  const inFlight = useRef<{ id: number; signed: boolean } | null>(null)
  // A transaction went through, even one the creator backed out of.
  const enabled = useRef(false)
  // The status shown is a backed-out transaction picked back up.
  const resumed = useRef(false)

  function start() {
    if (enabled.current) {
      setStep('done')
      return
    }
    if (inFlight.current) {
      // Resume the backed-out transaction instead of sending a second one.
      attempt.current = inFlight.current.id
      resumed.current = true
      setPhase(inFlight.current.signed ? 'pending' : 'confirm')
      if (!social) setStep('enabling')
      return
    }
    const sent = { id: ++attempt.current, signed: false }
    inFlight.current = sent
    resumed.current = false
    setPhase('confirm')
    if (!social) setStep('enabling')
    enableSales.mutate(
      {
        collection,
        onSigned: () => {
          sent.signed = true
          if (attempt.current === sent.id) setPhase('pending')
        }
      },
      {
        onSuccess: () => {
          if (inFlight.current === sent) inFlight.current = null
          enabled.current = true
          if (attempt.current === sent.id) setStep('done')
        },
        onError: cause => {
          if (inFlight.current === sent) inFlight.current = null
          if (attempt.current !== sent.id) return
          setStep('enable')
          const failure = toSellItemError(cause).reason
          // Dismissing the wallet prompt is the creator changing their mind, not a failure to report.
          if (failure !== 'rejected') onFailed(failure)
        }
      }
    )
  }

  function backOut() {
    attempt.current++
    // Signing it later only repeats setMinters, which is redundant rather than harmful.
    if (resumed.current && inFlight.current && !inFlight.current.signed) inFlight.current = null
    resumed.current = false
    setStep('enable')
  }

  let modal: ReactElement | null = null
  if (!salesEnabled && step === 'enable') {
    modal = (
      <EnableSalesModal reason={reason} busy={social && enableSales.isPending} onCancel={onClose} onConfirm={start} />
    )
  } else if (!salesEnabled && step === 'enabling') {
    modal = (
      <PendingModal
        label={phase === 'confirm' ? t('sell_item_modal.confirm_in_wallet') : t('sell_item_modal.enable_sales.pending')}
        onCancel={phase === 'confirm' ? backOut : undefined}
        testId="enable-sales-pending"
      />
    )
  }

  return { modal, isPending: enableSales.isPending }
}
