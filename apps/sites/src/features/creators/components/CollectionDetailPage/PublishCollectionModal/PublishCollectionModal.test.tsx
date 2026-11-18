import { beforeEach, describe, expect, it, vi } from 'vitest'
import { type ComponentProps } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { type Session } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { PublishCollectionError } from '~/lib/publishCollection'
import { ItemType, type Item } from '~/lib/items'
import { ValidationSeverity, type ValidationIssue } from '~/lib/validation'
import { PublishCollectionModal } from './PublishCollectionModal'

const rerun = vi.fn()
vi.mock('~/hooks/useCollection', () => ({
  useAllCollectionItems: () => ({ data: [], isLoading: false }),
  useSaveCollection: () => ({ mutate: vi.fn(), isPending: false, error: null })
}))
const profile = vi.hoisted(() => ({ data: undefined as { email?: string } | undefined, isLoading: false }))
vi.mock('~/hooks/useProfile', () => ({ useProfile: () => profile }))
vi.mock('~/hooks/useCollectionValidation', () => ({ useRerunItemValidation: () => rerun }))
vi.mock('~/lib/analytics', () => ({ track: vi.fn() }))
vi.mock('~/hooks/useItemSize', () => ({
  useCatalystItemSizes: () => ({ sizes: new Map(), unknown: new Set(), isMeasuring: false })
}))
vi.mock('./ConfirmNameStep', () => ({
  ConfirmNameStep: (props: { initialEmail: string; onConfirm: (details: { name: string; email: string }) => void }) => (
    <div data-testid="confirm-name-step" data-email={props.initialEmail}>
      <button
        data-testid="confirm-details"
        onClick={() => props.onConfirm({ name: 'Halloween', email: 'sam@example.org' })}
      />
    </div>
  )
}))
vi.mock('./ConfirmItemsStep', () => ({
  ConfirmItemsStep: (props: { onBack: () => void; onConfirm: () => void }) => (
    <div>
      <button data-testid="items-back" onClick={props.onBack} />
      <button data-testid="items-confirm" onClick={props.onConfirm} />
    </div>
  )
}))
vi.mock('./PaymentStep', () => ({
  PaymentStep: (props: { email: string; onFailed: (error: PublishCollectionError) => void }) => (
    <div data-testid="payment-step" data-email={props.email}>
      <button data-testid="tos-fails" onClick={() => props.onFailed(new PublishCollectionError('tos_failed'))} />
    </div>
  )
}))
vi.mock('./TopUpOutcome', () => ({ TopUpOutcome: () => null }))

const collection = { id: 'c1', name: 'Halloween' } as Collection
const session = { address: '0xabc' } as Session

function makeItem(id: string): Item {
  return {
    id,
    name: `Item ${id}`,
    description: '',
    thumbnail: 'thumbnail.png',
    owner: '0xabc',
    isPublished: false,
    isApproved: false,
    inCatalyst: false,
    type: ItemType.WEARABLE,
    data: { category: 'hat', representations: [] },
    contents: {},
    createdAt: 1,
    updatedAt: 1
  }
}

const error: ValidationIssue = { code: 'skeleton', severity: ValidationSeverity.ERROR, message: 'Wrong skeleton' }
const broken = makeItem('broken')
const clean = makeItem('clean')

type Props = ComponentProps<typeof PublishCollectionModal>
const onClose = vi.fn()

function renderModal(overrides: Partial<Props> = {}) {
  const props: Props = {
    collection,
    session,
    validation: { isValidating: true, results: [] },
    blockOnErrors: false,
    onClose,
    onPublished: vi.fn(),
    ...overrides
  }
  const view = render(<PublishCollectionModal {...props} />, { wrapper: TranslationProvider })
  return {
    ...view,
    update: (next: Partial<Props>) => view.rerender(<PublishCollectionModal {...props} {...next} />)
  }
}

beforeEach(() => {
  profile.data = undefined
  profile.isLoading = false
  onClose.mockReset()
  rerun.mockReset().mockResolvedValue([])
})

describe('PublishCollectionModal item checks', () => {
  it('waits for the checks and goes straight to the first step when every item is clean', () => {
    const { update } = renderModal()
    expect(screen.getByTestId('publish-validating')).toBeInTheDocument()
    update({ validation: { isValidating: false, results: [{ item: clean, issues: [] }] } })
    expect(screen.getByTestId('confirm-name-step')).toBeInTheDocument()
  })

  it('holds the wizard behind the issues view until the creator continues', async () => {
    renderModal({ validation: { isValidating: false, results: [{ item: broken, issues: [error] }] } })
    expect(screen.queryByTestId('confirm-name-step')).not.toBeInTheDocument()
    await userEvent.click(screen.getByTestId('publish-validation-continue'))
    expect(screen.getByTestId('confirm-name-step')).toBeInTheDocument()
  })

  it('skips the checks when coming back from buying credits', () => {
    renderModal({
      resume: { paymentMethod: 'credits', termsAccepted: true, email: 'jane.doe@example.com', orderId: null }
    })
    expect(screen.queryByTestId('publish-validating')).not.toBeInTheDocument()
    expect(screen.getByTestId('payment-step')).toBeInTheDocument()
  })

  it('asks for the email again when the credits purchase was started without one', () => {
    renderModal({ resume: { paymentMethod: 'credits', termsAccepted: true, email: null, orderId: null } })
    expect(screen.queryByTestId('payment-step')).not.toBeInTheDocument()
    expect(screen.getByTestId('confirm-name-step')).toBeInTheDocument()
  })
})

describe('PublishCollectionModal contact email', () => {
  const ready = { validation: { isValidating: false, results: [] } }

  it('waits for the profile, then offers its email', () => {
    profile.isLoading = true
    const { update } = renderModal(ready)
    expect(screen.getByTestId('publish-name-loading')).toBeInTheDocument()
    profile.isLoading = false
    profile.data = { email: ' jane.doe@example.com ' }
    update(ready)
    expect(screen.getByTestId('confirm-name-step')).toHaveAttribute('data-email', 'jane.doe@example.com')
  })

  it('keeps the confirmed email when going back and hands it to the payment step', async () => {
    profile.data = { email: 'jane.doe@example.com' }
    renderModal(ready)
    await userEvent.click(screen.getByTestId('confirm-details'))
    await userEvent.click(screen.getByTestId('items-back'))
    expect(screen.getByTestId('confirm-name-step')).toHaveAttribute('data-email', 'sam@example.org')
    await userEvent.click(screen.getByTestId('confirm-details'))
    await userEvent.click(screen.getByTestId('items-confirm'))
    expect(screen.getByTestId('payment-step')).toHaveAttribute('data-email', 'sam@example.org')
  })

  it('sends the creator back to the email when the ToS record fails', async () => {
    renderModal({
      resume: { paymentMethod: 'credits', termsAccepted: true, email: 'jane.doe@example.com', orderId: null }
    })
    await userEvent.click(screen.getByTestId('tos-fails'))
    await userEvent.click(screen.getByTestId('publish-error-retry'))
    expect(screen.getByTestId('confirm-name-step')).toHaveAttribute('data-email', 'jane.doe@example.com')
  })
})
