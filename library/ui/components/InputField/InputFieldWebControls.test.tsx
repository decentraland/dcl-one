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

describe('when a shared web field receives an ARIA invalid state', () => {
  it.each([
    [true, 'true'],
    ['true', 'true'],
    ['false', null]
  ] as const)('should reflect %s on both field frames', (invalid, expected) => {
    render(
      <>
        <WebSelect aria-label="Reason" aria-invalid={invalid} />
        <WebTextarea aria-label="Description" aria-invalid={invalid} />
      </>
    )
    expect(
      screen.getByRole('combobox', { name: 'Reason' }).parentElement?.getAttribute('data-invalid')
    ).toBe(expected)
    expect(
      screen
        .getByRole('textbox', { name: 'Description' })
        .parentElement?.getAttribute('data-invalid')
    ).toBe(expected)
  })
})

describe('when a shared web field has no label, description, error or hint', () => {
  it.each([
    ['select', () => <WebSelect aria-label="Reason" rootClassName="placed" className="own" />],
    ['textarea', () => <WebTextarea aria-label="Reason" rootClassName="placed" className="own" />]
  ])('should render the %s inside its frame alone, placed by rootClassName', (_, field) => {
    const { container } = render(field())
    const control = container.querySelector('select, textarea')!
    expect(container.querySelector('label')).toBeNull()
    expect(container.firstElementChild).toBe(control.parentElement)
    expect(control.parentElement).toHaveClass('placed')
    expect(control).toHaveClass('own')
    expect(control).not.toHaveAttribute('id')
    expect(control).not.toHaveAttribute('aria-describedby')
  })

  it.each([
    ['null', null],
    ['false', false]
  ] as const)('should treat a %s label as no label', (_, label) => {
    const { container } = render(<WebSelect aria-label="Reason" label={label} />)
    const select = screen.getByRole('combobox', { name: 'Reason' })
    expect(container.querySelector('label')).toBeNull()
    expect(container.firstElementChild).toBe(select.parentElement)
    expect(select).not.toHaveAttribute('id')
    expect(select).not.toHaveAttribute('aria-describedby')
  })

  it('should not accept messages beside a label that may be missing', () => {
    const maybe = undefined as string | undefined
    // @ts-expect-error A label that may be missing could drop the error, so the type refuses it.
    render(<WebSelect aria-label="Reason" label={maybe} error="Choose a reason" />)
    expect(screen.queryByText('Choose a reason')).toBeNull()
  })

  it('should not accept a description, error or hint without a label', () => {
    // @ts-expect-error The messages need the label, so a field without one renders the control alone.
    const { container } = render(<WebSelect aria-label="Reason" error="Choose a reason" />)
    expect(container.querySelector('label')).toBeNull()
    expect(screen.queryByText('Choose a reason')).toBeNull()
  })
})

describe('when a labelled shared web field has blank messages', () => {
  it('should leave out the blank description, error and hint lines', () => {
    render(<WebSelect label="Reason" description=" " error="" hint="  " />)
    const select = screen.getByRole('combobox', { name: 'Reason' })
    expect(select).not.toHaveAttribute('aria-describedby')
    expect(select).not.toHaveAttribute('aria-invalid')
    expect(select.parentElement).not.toHaveAttribute('data-invalid')
  })
})

describe('when an error appears on a labelled shared web field that has focus', () => {
  it('should keep the same control focused and describe it with the new error', async () => {
    const user = userEvent.setup()
    const { rerender } = render(<WebTextarea label="Details" />)
    const textarea = screen.getByRole('textbox', { name: 'Details' })
    await user.click(textarea)
    rerender(<WebTextarea label="Details" error="Add a few words" />)
    expect(screen.getByRole('textbox', { name: 'Details', description: 'Add a few words' })).toBe(textarea)
    expect(textarea).toHaveFocus()
    rerender(<WebTextarea label="Details" />)
    expect(textarea).not.toHaveAttribute('aria-invalid')
    expect(textarea).toHaveFocus()
  })
})

describe('when a shared web select has a visible label, description and error', () => {
  it('should name the control by its label and describe it with the caller, description and error lines', () => {
    render(
      <>
        <WebSelect
          label="Reason"
          description="Pick the closest one."
          error="Choose a reason"
          aria-describedby="caller"
          rootClassName="placed"
          className="own"
        >
          <option value="">None</option>
        </WebSelect>
        <p id="caller">Caller note.</p>
      </>
    )
    const select = screen.getByRole('combobox', {
      name: 'Reason',
      description: 'Caller note. Pick the closest one. Choose a reason'
    })
    expect(select).toHaveAttribute('aria-invalid', 'true')
    expect(select.parentElement).toHaveAttribute('data-invalid', 'true')
    expect(select).toHaveClass('own')
    expect(select.parentElement?.parentElement).toHaveClass('placed')
  })

  it('should keep the caller id and let an error override a caller aria-invalid of false', () => {
    render(
      <WebSelect id="reason" label="Reason" aria-invalid="false" error="Choose a reason">
        <option value="">None</option>
      </WebSelect>
    )
    const select = screen.getByRole('combobox', { name: 'Reason' })
    expect(select).toHaveAttribute('id', 'reason')
    expect(select).toHaveAttribute('aria-invalid', 'true')
  })

  it('should keep a caller aria-invalid when there is no error line', () => {
    render(<WebSelect label="Reason" aria-invalid />)
    const select = screen.getByRole('combobox', { name: 'Reason' })
    expect(select).toHaveAttribute('aria-invalid', 'true')
    expect(select.parentElement).toHaveAttribute('data-invalid', 'true')
  })
})

describe('when a shared web textarea has a label given as elements with a required mark', () => {
  it('should leave the hidden mark out of its name and focus the control when the label is clicked', async () => {
    const user = userEvent.setup()
    render(
      <WebTextarea
        label={
          <span>
            Description <span aria-hidden="true">*</span>
          </span>
        }
        hint="Up to 500 characters."
        required
      />
    )
    const textarea = screen.getByRole('textbox', {
      name: 'Description',
      description: 'Up to 500 characters.'
    })
    expect(textarea).toBeRequired()
    await user.click(screen.getByText('Description'))
    expect(textarea).toHaveFocus()
  })

  it('should keep a hidden label as the name without showing it', () => {
    render(<WebTextarea label="Notes" labelHidden />)
    const textarea = screen.getByRole('textbox', { name: 'Notes' })
    expect((textarea as HTMLTextAreaElement).labels[0]).toHaveAttribute('data-hidden', 'true')
  })

  it('should give two labelled fields their own ids', () => {
    render(
      <>
        <WebTextarea label="First" />
        <WebTextarea label="Second" />
      </>
    )
    const first = screen.getByRole('textbox', { name: 'First' })
    const second = screen.getByRole('textbox', { name: 'Second' })
    expect(first.id).not.toBe(second.id)
  })
})
