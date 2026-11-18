import { useEffect, useMemo } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { useTranslation } from '~/intl'
import { useCreditsOrderOutcome } from '~/hooks/useCreditsOrderOutcome'
import { track } from '~/lib/analytics'
import { formatCredits } from '~/lib/publishFee'
import { ConfirmModal } from '~/components/ConfirmModal'
import { useCreditPacks } from '~/hooks/useCreditPacks'
import { CREDIT_PACKS } from '~/lib/creditPacks'
import errorArt from '~/assets/modal-error.png'
import { PendingModal } from '~/components/PendingModal'
import { artForCredits } from './packArt'

type Props = {
  address: string
  orderId: string
  onDone: () => void
}

/**
 * What became of the credits bought on the way back from Stripe, stacked over the payment step: a
 * spinner while the webhook lands, then the purchase's success, pending or failure dialog.
 */
export function TopUpOutcome({ address, orderId, onDone }: Props) {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const outcome = useCreditsOrderOutcome(address, orderId)
  const { status } = outcome
  const creditsGranted = 'creditsGranted' in outcome ? outcome.creditsGranted : undefined
  const { packs } = useCreditPacks()
  // The illustration of the pack just bought; the pending dialog knows no amount yet, so it shows the entry pack.
  const creditsArt = useMemo(() => artForCredits(packs ?? CREDIT_PACKS, creditsGranted ?? 0), [packs, creditsGranted])

  useEffect(() => {
    if (status === 'confirming') return
    track('Credits checkout settled', { orderId, status, credits: creditsGranted ?? null })
    // The card was charged in every settled state but these two: the balance is worth re-reading.
    if (status !== 'abandoned' && status !== 'initiated') {
      void queryClient.invalidateQueries({ queryKey: ['credits-balance', address] })
    }
  }, [status, creditsGranted, orderId, address, queryClient])

  // Nothing was paid: the wizard is already where the creator left it.
  useEffect(() => {
    if (status === 'abandoned' || status === 'initiated') onDone()
  }, [status, onDone])

  if (status === 'confirming') {
    return <PendingModal label={t('publish_collection_modal.top_up.confirming')} testId="top-up-pending" />
  }
  if (status === 'credited') {
    return (
      <ConfirmModal
        title={t('publish_collection_modal.top_up.success_title')}
        description={t('publish_collection_modal.top_up.success_description', {
          credits: formatCredits(creditsGranted ?? 0)
        })}
        art={{ src: creditsArt }}
        celebrate
        onClose={onDone}
        confirm={{ label: t('publish_collection_modal.top_up.done'), onClick: onDone, testId: 'top-up-success-done' }}
        testId="top-up-success"
      />
    )
  }
  if (status === 'failed') {
    return (
      <ConfirmModal
        title={t('publish_collection_modal.top_up.failed_title')}
        description={t('publish_collection_modal.top_up.failed_description')}
        art={{ src: errorArt }}
        onClose={onDone}
        confirm={{ label: t('publish_collection_modal.top_up.done'), onClick: onDone, testId: 'top-up-failed-done' }}
        testId="top-up-failed"
      />
    )
  }
  if (status === 'pending') {
    return (
      <ConfirmModal
        title={t('publish_collection_modal.top_up.pending_title')}
        description={t('publish_collection_modal.top_up.pending_description')}
        art={{ src: creditsArt }}
        onClose={onDone}
        confirm={{ label: t('publish_collection_modal.top_up.done'), onClick: onDone, testId: 'top-up-pending-done' }}
        testId="top-up-pending-outcome"
      />
    )
  }
  return null
}
