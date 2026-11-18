import { useCallback, useState } from 'react'
import { useTranslation } from '@/data/lib/creators/intl'
import { useAllCollectionItems, useSaveCollection } from '../../../hooks/useCollection'
import { useProfile } from '../../../hooks/useProfile'
import { track } from '@/data/lib/creators/analytics'
import type { Session } from '@/data/lib/creators/auth'
import { isCollectionLocked, type Collection } from '@/data/lib/creators/collections'
import type { TopUpResume } from '@/data/lib/creators/creditsTopUp'
import type { PaymentMethod, PublishCollectionError, PublishResult } from '@/data/lib/creators/publishCollection'
import { Modal } from '../../Modal'
import { ConfirmItemsStep } from './ConfirmItemsStep'
import { ConfirmNameStep } from './ConfirmNameStep'
import { PaymentStep } from './PaymentStep'
import { PublishErrorModal } from './PublishErrorModal'
import { TopUpOutcome } from './TopUpOutcome'
import { ValidationGate } from './ValidationGate'
import type { ItemCheck } from '@/data/lib/creators/validation'
import { StepIndicator } from '../../StepIndicator'
import styles from './PublishCollectionModal.module.css'
import shared from '../../../styles/shared.module.css'

enum Step {
  Name = 1,
  Items,
  Payment
}

const TOTAL_STEPS = 3

/** Where a creator back from buying credits picks the wizard up: the payment step, with the order to settle. */
export type PublishResume = Pick<TopUpResume, 'paymentMethod' | 'termsAccepted' | 'email'> & {
  /** Null when the checkout was cancelled: nothing to wait for. */
  orderId: string | null
}

type Props = {
  collection: Collection
  session: Session
  resume?: PublishResume
  /** The collection's item checks, run by the page; items still loading count as validating. */
  validation: { isValidating: boolean; results: ItemCheck[] }
  /** Errors block publishing (`block-publish-on-validation-errors`): no way past the issues view while any remain. */
  blockOnErrors: boolean
  onClose: () => void
  onPublished: (result: PublishResult) => void
}

/**
 * The three-step publish wizard: confirm name → review items → confirm & pay, after a pre-step that waits for
 * the item checks and shows anything they found. A failed publish swaps in the error dialog; TRY AGAIN comes
 * back here on the payment step with everything kept.
 */
export function PublishCollectionModal({
  collection,
  session,
  resume,
  validation,
  blockOnErrors,
  onClose,
  onPublished
}: Props) {
  const { t } = useTranslation()
  const { address } = session

  // Before the wizard: wait for the item checks and show what they found. Resuming skips them.
  const [gated, setGated] = useState(!resume)

  // A record from before step 1 asked for the email can't publish: send the creator back for it.
  const [step, setStep] = useState<Step>(resume?.email ? Step.Payment : Step.Name)
  const [email, setEmail] = useState(resume?.email ?? '')
  const [error, setError] = useState<PublishCollectionError | null>(null)
  const [paymentMethod, setPaymentMethod] = useState<PaymentMethod | null>(resume?.paymentMethod ?? null)
  const [termsAccepted, setTermsAccepted] = useState(resume?.termsAccepted ?? false)
  const [isStepBusy, setStepBusy] = useState(false)
  const [settlingOrder, setSettlingOrder] = useState<string | null>(resume?.orderId ?? null)
  const settleOrder = useCallback(() => setSettlingOrder(null), [])

  const itemsQuery = useAllCollectionItems(address, collection.id)
  const items = itemsQuery.data ?? []
  const saveCollection = useSaveCollection(address)
  const profile = useProfile(address)
  const profileEmail = profile.data?.email?.trim() ?? ''

  function confirmDetails(details: { name: string; email: string }) {
    const { name } = details
    setEmail(details.email)
    // Tracked once the step is actually left, so a failed rename retried N times counts once.
    const advance = () => {
      track('Publish details confirmed', {
        collectionId: collection.id,
        nameChanged: name !== collection.name,
        emailSource: !profileEmail ? 'typed' : details.email === profileEmail ? 'profile' : 'edited'
      })
      setStep(Step.Items)
    }
    if (name === collection.name) {
      advance()
      return
    }
    saveCollection.mutate({ ...collection, name }, { onSuccess: advance })
  }

  if (gated) {
    return (
      <ValidationGate
        collectionId={collection.id}
        flow="publish"
        validation={validation}
        blockOnErrors={blockOnErrors}
        onPass={() => setGated(false)}
        onClose={onClose}
      />
    )
  }

  if (error) {
    return (
      <PublishErrorModal
        reason={error.reason}
        onCancel={onClose}
        onRetry={() => {
          setError(null)
          // The server refusing the ToS record most likely means the email: send the creator back to it.
          setStep(error.reason === 'tos_failed' ? Step.Name : Step.Payment)
        }}
      />
    )
  }

  const busy = saveCollection.isPending || isStepBusy
  // builder-server refuses every save while the publish lock holds, so a retry can only confirm what was sent.
  const readOnly = isCollectionLocked(collection)

  return (
    <Modal
      title={t('publish_collection_modal.title')}
      size="large"
      onClose={onClose}
      closeDisabled={busy}
      testId="publish-collection-modal"
    >
      <div className={styles.main}>
        <StepIndicator current={step} total={TOTAL_STEPS} testId="publish-steps" />
        {step === Step.Name &&
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
              nameDisabled={readOnly}
              onCancel={onClose}
              onConfirm={confirmDetails}
            />
          ))}
        {step === Step.Items &&
          (itemsQuery.isLoading ? (
            <p className={styles.inlineNote} data-testid="publish-items-loading">
              <span className={shared.spinner} aria-hidden />
            </p>
          ) : (
            <ConfirmItemsStep
              address={address}
              collection={collection}
              items={items}
              readOnly={readOnly}
              onBusyChange={setStepBusy}
              onBack={() => setStep(Step.Name)}
              onConfirm={() => setStep(Step.Payment)}
            />
          ))}
        {step === Step.Payment && (
          <PaymentStep
            collection={collection}
            items={items}
            session={session}
            paymentMethod={paymentMethod}
            onPaymentMethodChange={setPaymentMethod}
            email={email}
            accepted={termsAccepted}
            onAcceptedChange={setTermsAccepted}
            onBusyChange={setStepBusy}
            onBack={() => setStep(Step.Items)}
            onPublished={onPublished}
            onFailed={setError}
          />
        )}
      </div>
      {settlingOrder && <TopUpOutcome address={address} orderId={settlingOrder} onDone={settleOrder} />}
    </Modal>
  )
}
