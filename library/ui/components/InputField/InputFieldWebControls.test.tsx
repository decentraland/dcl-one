import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'

import { WebSelect, WebTextarea } from './InputFieldWebControls'

describe('when a reader chooses a value in a shared web select', () => {
  it('should offer the available option and preserve the invalid state for assistive technology', async () => {
    const user = userEvent.setup()
    render(
      <WebSelect aria-label="Reason" aria-invalid defaultValue="">
        <option value="" disabled>
          Choose a reason
        </option>
        <option value="harassment">Harassment</option>
      </WebSelect>
    )
    const select = screen.getByRole('combobox', { name: 'Reason' })
    expect(screen.getByRole('option', { name: 'Choose a reason' })).toBeDisabled()
    expect(select).toHaveAttribute('aria-invalid', 'true')
    await user.selectOptions(select, 'harassment')
    expect(select).toHaveValue('harassment')
  })
})

describe('when a reader edits a shared web textarea', () => {
  it('should support multiline entry and expose the field description', async () => {
    const user = userEvent.setup()
    render(
      <>
        <WebTextarea aria-label="Description" aria-describedby="hint" rows={4} />
        <p id="hint">Describe what happened.</p>
      </>
    )
    const textarea = screen.getByRole('textbox', {
      name: 'Description',
      description: 'Describe what happened.'
    })
    await user.type(textarea, 'First line{enter}Second line')
    expect(textarea).toHaveValue('First line\nSecond line')
  })
})
