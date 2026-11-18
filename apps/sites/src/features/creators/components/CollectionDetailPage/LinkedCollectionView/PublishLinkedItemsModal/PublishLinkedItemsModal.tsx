import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from '@/data/lib/creators/intl'
import { useSaveCollection } from '@/features/creators/hooks/useCollection'
import { useItemCurations } from '@/features/creators/hooks/useLinkedCollection'
import { useLinkedSlots, usePublishLinkedItems, useSignSlotsCheque, useThirdPartyPrice } from '@/features/creators/hooks/useLinkedPublish'
import { useProfile } from '@/features/creators/hooks/useProfile'
import { useBeforeUnloadGuard } from '@/features/creators/hooks/useBeforeUnloadGuard'
import { track } from '@/data/lib/creators/analytics'
import { type Session } from '@/data/lib/creators/auth'
import { type SlotsCheque } from '@/data/lib/creators/builder'
import { type Collection } from '@/data/lib/creators/collections'
import { type Item } from '@/data/lib/creators/items'
import { loadPendingPublish, savePendingPublish, type PendingPublish } from '@/data/lib/creators/pendingLinkedPayment'
import { getMaticChainId } from '@/data/lib/creators/publishCollection'
import {
  LinkedPublishError,
  getLinkedPublishPhase,
  getLinkedPublishPhases,
  getLinkedPublishQuote,
  getPublishState,
  readFreshLinkedQuote,
  type LinkedPublishStep
} from '@/data/lib/creators/publishLinkedItems'
import { LINKED_COLLECTION_NAME_MAX_LENGTH, type ThirdParty } from '@/data/lib/creators/thirdParty'
import { Modal } from '@/features/creators/components/Modal'
import { PendingModal } from '@/features/creators/components/PendingModal'
import { StepIndicator } from '@/features/creators/components/StepIndicator'
import { ConfirmNameStep } from '../../PublishCollectionModal/ConfirmNameStep'
import styles from '@/features/creators/components/CollectionDetailPage/PublishCollectionModal/PublishCollectionModal.module.css'
import shared from '@/features/creators/styles/shared.module.css'
import { LinkedItemsStep } from './LinkedItemsStep'
import { LinkedKindStep } from './LinkedKindStep'
import { LinkedPaymentStep } from './LinkedPaymentStep'
import { LinkedPublishOutcomeModal } from './LinkedPublishOutcomeModal'

type Step = 'name' | 'items' | 'kind' | 'payment'

type Props = {
  collection: Collection
  thirdParty: ThirdParty
  session: Session
  itemsToPublish: Item[]
  itemsToPush: Item[]
  onClose: () => void
  onPublished: () => void
}

const NOTHING: Item[] = []

/**
 * The linked publish wizard (legacy PublishWizardCollectionModal for third parties): confirm name and email →
 * confirm items and sign the slots cheque → pick the third party's kind on its first publish → pay the slots in
 * MANA and submit for review.
 */
export function PublishLinkedItemsModal({
  collection,
  thirdParty,
  session,
  itemsToPublish: selectedToPublish,
  itemsToPush,
  onClose,
  onPublished
}: Props) {
  const { t } = useTranslation()
  const { address } = session
  const needsKind = !thirdParty.published
  const steps = useMemo<Step[]>(
    () => ['name', 'items', ...(needsKind ? (['kind'] as const) : []), 'payment'],
    [needsKind]
  )
  const [step, setStep] = useState<Step>('name')
  const [email, setEmail] = useState('')
  const [cheque, setCheque] = useState<SlotsCheque | null>(null)
  const [isProgrammatic, setProgrammatic] = useState(thirdParty.isProgrammatic)
  const [termsAccepted, setTermsAccepted] = useState(false)
  const [runningStep, setRunningStep] = useState<LinkedPublishStep | null>(null)
  const [outcome, setOutcome] = useState<'success' | LinkedPublishError | null>(null)
  // A payment an earlier attempt sent, which a retry must resume from (kept across reopenings of the wizard).
  const pendingScope = useMemo(
    () => ({ chainId: getMaticChainId(), address, collectionId: collection.id }),
    [address, collection.id]
  )
  const [pending, setPending] = useState<PendingPublish>(() => loadPendingPublish(pendingScope))
  const pendingRef = useRef(pending)
  // Written to storage right away, not in a render: the run reports its payment even if the wizard is gone by then.
  const updatePending = useCallback(
    (patch: PendingPublish) => {
      const next = { ...pendingRef.current, ...patch }
      pendingRef.current = next
      savePendingPublish(pendingScope, next)
      setPending(next)
    },
    [pendingScope]
  )
  const paidTx = pending.payment
  // Set synchronously on Submit: a second click during the slots re-read must not start a second, paying run.
  const submitting = useRef(false)
  const [slotsError, setSlotsError] = useState(false)
  const [feeChanged, setFeeChanged] = useState(false)

  const profile = useProfile(address)
  const profileEmail = profile.data?.email?.trim() ?? ''
  const saveCollection = useSaveCollection(address)
  const signCheque = useSignSlotsCheque(session)
  const price = useThirdPartyPrice()
  const slots = useLinkedSlots(address, thirdParty.id)
  const publish = usePublishLinkedItems(session, collection)
  // Items an earlier attempt already published (its push failing afterwards, or its answer lost) are neither quoted
  // nor paid for again: a retry only finishes what is left.
  const liveCurations = useItemCurations(address, collection)
  const [publishDone, setPublishDone] = useState(false)
  const itemsToPublish = useMemo(
    () =>
      publishDone ||
      (liveCurations.data && getPublishState(selectedToPublish, liveCurations.data) === 'already_published')
        ? NOTHING
        : selectedToPublish,
    [publishDone, liveCurations.data, selectedToPublish]
  )
  const isRunning = publish.isPending || runningStep !== null
  // Whether this run buys slots or resumes a payment, known once the fee is re-read on Submit.
  const [runPays, setRunPays] = useState(false)
  const phases = useMemo(
    () => (runningStep ? getLinkedPublishPhases(runPays, runningStep) : []),
    [runPays, runningStep]
  )
  useBeforeUnloadGuard(isRunning)

  useEffect(() => {
    track('Publish TP Items started', {
      collectionId: collection.id,
      thirdPartyId: thirdParty.id,
      items: selectedToPublish.length,
      pushed: itemsToPush.length
    })
    // Once per wizard opened.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const stepIndex = steps.indexOf(step)
  const goTo = useCallback((next: Step) => setStep(next), [])
  const back = () => goTo(steps[Math.max(0, stepIndex - 1)])
  const forward = () => goTo(steps[Math.min(steps.length - 1, stepIndex + 1)])

  function confirmDetails(details: { name: string; email: string }) {
    setEmail(details.email)
    if (details.name === collection.name || collection.isPublished) return forward()
    saveCollection.mutate({ ...collection, name: details.name }, { onSuccess: forward })
  }

  function confirmItems() {
    if (itemsToPublish.length === 0 || cheque) return forward()
    signCheque.mutate(
      { thirdPartyId: thirdParty.id, qty: itemsToPublish.length },
      {
        onSuccess: signed => {
          track('Sign TP cheque', { collectionId: collection.id, thirdPartyId: thirdParty.id, qty: signed.qty })
          setCheque(signed)
          forward()
        }
      }
    )
  }

  async function submit() {
    if (!price.data || !slots.data || submitting.current) return
    submitting.current = true
    setSlotsError(false)
    setFeeChanged(false)
    setRunningStep('check')
    // A failed read hands back the stale value, so it counts as no read at all.
    const fresh = await readFreshLinkedQuote(
      itemsToPublish.length,
      { isProgrammatic, published: thirdParty.published },
      { slots: slots.data, price: price.data },
      {
        readSlots: async () => {
          const read = await slots.refetch()
          return read.isError ? undefined : read.data
        },
        readPrice: async () => {
          const read = await price.refetch()
          return read.isError ? undefined : read.data
        }
      }
    )
    if (fresh.kind !== 'ready') {
      if (fresh.kind === 'read_failed') setSlotsError(true)
      else setFeeChanged(true)
      submitting.current = false
      setRunningStep(null)
      return
    }
    setRunPays(
      !!paidTx ||
        getLinkedPublishQuote(itemsToPublish.length, fresh.slots, fresh.price, {
          isProgrammatic,
          published: thirdParty.published
        }).slotsToBuy > 0
    )
    publish.mutate(
      {
        collection,
        thirdParty,
        itemsToPublish,
        itemsToPush,
        cheque,
        email,
        isProgrammatic,
        slots: fresh.slots,
        maxFeeManaWei: fresh.maxFeeManaWei,
        price: fresh.price,
        paidTx,
        onStep: setRunningStep,
        onPayment: payment => updatePending({ payment })
      },
      {
        onSuccess: () => {
          updatePending({ payment: undefined })
          setOutcome('success')
          onPublished()
        },
        onError: error => {
          const failure =
            error instanceof LinkedPublishError ? error : new LinkedPublishError('publish', 'generic', String(error))
          // Failing at the push means the items were published: the list refreshes, and a retry only pushes.
          if (failure.reason === 'push_failed') {
            setPublishDone(true)
            onPublished()
          }
          setOutcome(failure)
        },
        onSettled: () => {
          submitting.current = false
          setRunningStep(null)
        }
      }
    )
  }

  if (outcome) {
    return (
      <LinkedPublishOutcomeModal
        outcome={outcome}
        onClose={onClose}
        onRetry={() => {
          setOutcome(null)
          // The server refusing the ToS most likely means the email: back to the first step.
          goTo(outcome !== 'success' && outcome.reason === 'tos_failed' ? 'name' : 'payment')
        }}
        onForgetPayment={() => {
          updatePending({ payment: undefined })
          setOutcome(null)
          goTo('payment')
        }}
      />
    )
  }

  if (runningStep) {
    return (
      <PendingModal
        title={t('publish_linked_modal.running.title')}
        label={t(`publish_linked_modal.running.${runningStep}`)}
        steps={{
          labels: phases.map(phase => t(`publish_linked_modal.running.phase.${phase}`)),
          current: phases.indexOf(getLinkedPublishPhase(runningStep)) + 1
        }}
        testId="publish-linked-running"
      />
    )
  }

  const busy = saveCollection.isPending || signCheque.isPending

  return (
    <Modal
      title={t('publish_linked_modal.title')}
      size="large"
      onClose={onClose}
      closeDisabled={busy}
      testId="publish-linked-modal"
    >
      <div className={styles.main}>
        <StepIndicator current={stepIndex + 1} total={steps.length} testId="publish-linked-steps" />
        {step === 'name' &&
          (profile.isLoading ? (
            <p className={styles.inlineNote} data-testid="publish-name-loading">
              <span className={shared.spinner} aria-hidden />
            </p>
          ) : (
            <ConfirmNameStep
              initialName={collection.name}
              initialEmail={email || profileEmail}
              isSaving={saveCollection.isPending}
              saveError={saveCollection.error?.message ?? null}
              nameMaxLength={LINKED_COLLECTION_NAME_MAX_LENGTH}
              nameDisabled={collection.isPublished}
              onCancel={onClose}
              onConfirm={confirmDetails}
            />
          ))}
        {step === 'items' && (
          <LinkedItemsStep
            collection={collection}
            itemsToPublish={itemsToPublish}
            itemsToPush={itemsToPush}
            isSigning={signCheque.isPending}
            signError={signCheque.isError}
            onBack={back}
            onConfirm={confirmItems}
          />
        )}
        {step === 'kind' && (
          <LinkedKindStep
            value={isProgrammatic}
            price={price.data}
            priceError={price.isError}
            onRetryPrice={() => void price.refetch()}
            onChange={setProgrammatic}
            onBack={back}
            onConfirm={forward}
          />
        )}
        {step === 'payment' && (
          <LinkedPaymentStep
            collection={collection}
            thirdParty={thirdParty}
            session={session}
            itemsToPublish={itemsToPublish}
            itemsToPush={itemsToPush}
            isProgrammatic={isProgrammatic}
            price={price.data}
            priceError={price.isError}
            onRetryPrice={() => void price.refetch()}
            slots={slots.data}
            slotsError={slots.isError || slotsError}
            feeChanged={feeChanged}
            onRetrySlots={() => {
              setSlotsError(false)
              void slots.refetch()
            }}
            resuming={!!paidTx}
            busy={isRunning}
            accepted={termsAccepted}
            onAcceptedChange={setTermsAccepted}
            onBack={back}
            onSubmit={() => void submit()}
          />
        )}
      </div>
    </Modal>
  )
}
