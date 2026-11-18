import { describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { NAME_ALREADY_IN_USE_ERROR } from '~/lib/collections'
import { ConfirmNameStep } from './ConfirmNameStep'

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

function renderStep(props: Partial<React.ComponentProps<typeof ConfirmNameStep>> = {}) {
  const onConfirm = vi.fn()
  const onCancel = vi.fn()
  render(
    <ConfirmNameStep
      initialName="Halloween"
      initialEmail="jane.doe@example.com"
      isSaving={false}
      saveError={null}
      onCancel={onCancel}
      onConfirm={onConfirm}
      {...props}
    />,
    { wrapper }
  )
  return { onConfirm, onCancel }
}

describe('ConfirmNameStep', () => {
  it('keeps the confirm button disabled until the acknowledgment is checked', async () => {
    const { onConfirm } = renderStep()
    const confirm = screen.getByTestId('publish-name-confirm')
    expect(confirm).toBeDisabled()
    await userEvent.click(screen.getByTestId('publish-name-accept'))
    expect(confirm).toBeEnabled()
    await userEvent.click(confirm)
    expect(onConfirm).toHaveBeenCalledWith({ name: 'Halloween', email: 'jane.doe@example.com' })
  })

  it('lets the creator fix the name in place and confirms the trimmed value', async () => {
    const { onConfirm } = renderStep()
    const input = screen.getByTestId('publish-name-input')
    await userEvent.clear(input)
    await userEvent.type(input, '  Spooky Hats ')
    expect(screen.getByTestId('publish-name-count')).toHaveTextContent('14/32')
    expect(screen.getByTestId('publish-name-accept').parentElement).toHaveTextContent('"Spooky Hats"')
    await userEvent.click(screen.getByTestId('publish-name-accept'))
    await userEvent.click(screen.getByTestId('publish-name-confirm'))
    expect(onConfirm).toHaveBeenCalledWith({ name: 'Spooky Hats', email: 'jane.doe@example.com' })
  })

  it('blocks names with a colon', async () => {
    const { onConfirm } = renderStep()
    await userEvent.type(screen.getByTestId('publish-name-input'), ':x')
    await userEvent.click(screen.getByTestId('publish-name-accept'))
    expect(screen.getByTestId('publish-name-error')).toHaveTextContent(/":"/)
    expect(screen.getByTestId('publish-name-confirm')).toBeDisabled()
    expect(onConfirm).not.toHaveBeenCalled()
  })

  it('explains a taken name until the creator edits it', async () => {
    renderStep({ saveError: NAME_ALREADY_IN_USE_ERROR })
    expect(screen.getByTestId('publish-name-error')).toHaveTextContent(/already in use/)
    await userEvent.type(screen.getByTestId('publish-name-input'), '2')
    expect(screen.queryByTestId('publish-name-error')).not.toBeInTheDocument()
  })

  it('requires a contact email before continuing', async () => {
    const { onConfirm } = renderStep({ initialEmail: '' })
    await userEvent.click(screen.getByTestId('publish-name-accept'))
    expect(screen.getByTestId('publish-name-confirm')).toBeDisabled()
    await userEvent.type(screen.getByTestId('publish-email-input'), ' sam@example.org ')
    await userEvent.click(screen.getByTestId('publish-name-confirm'))
    expect(onConfirm).toHaveBeenCalledWith({ name: 'Halloween', email: 'sam@example.org' })
  })

  it('flags an invalid email once the creator leaves the field', async () => {
    const { onConfirm } = renderStep({ initialEmail: '' })
    const input = screen.getByTestId('publish-email-input')
    await userEvent.type(input, 'sam@example')
    expect(screen.queryByTestId('publish-email-error')).not.toBeInTheDocument()
    await userEvent.click(screen.getByTestId('publish-name-accept'))
    expect(input).toHaveAccessibleDescription(screen.getByTestId('publish-email-error').textContent ?? '')
    expect(screen.getByTestId('publish-name-confirm')).toBeDisabled()
    expect(onConfirm).not.toHaveBeenCalled()
  })
})
