import { createRef } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderToString } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { InputExplorerNames } from '../InputExplorerNames/InputExplorerNames'
import { InputField, type InputFieldProps } from './InputField'
function SharedWebField(props: InputFieldProps) {
  return <InputField {...props} surface="web" />
}

for (const Field of [InputExplorerNames, SharedWebField]) {
  describe(`when ${Field.name} receives native input data`, () => {
    it('should associate its label, description and error without removing a caller description', () => {
      render(
        <>
          <p id="external">External</p>
          <Field
            label="Email"
            description="Use an address"
            error="Invalid address"
            aria-describedby="external"
          />
        </>
      )
      const input = screen.getByRole('textbox', { name: 'Email' })
      expect(input).toHaveAttribute('aria-invalid', 'true')
      expect(input).toHaveAccessibleDescription('External Use an address Invalid address')
      expect(screen.getByText('Invalid address')).toBeInTheDocument()
      expect(screen.queryByRole('status')).toBeNull()
    })
    it('should preserve controlled values, native name and change events', async () => {
      const change = vi.fn()
      render(
        <Field
          label="Name"
          value="Ada"
          name="name"
          onChange={(event) => change(event.currentTarget.value)}
        />
      )
      await userEvent.setup().type(screen.getByRole('textbox'), 'X')
      expect(change).toHaveBeenCalledWith('AdaX')
      expect(screen.getByRole('textbox')).toHaveValue('Ada')
      expect(screen.getByRole('textbox')).toHaveAttribute('name', 'name')
    })
    it('should reveal passwords without submitting a surrounding form or losing their value', async () => {
      const submit = vi.fn((event) => event.preventDefault())
      render(
        <form onSubmit={submit}>
          <Field label="Password" type="password" defaultValue="secret" allowPasswordReveal />
        </form>
      )
      const user = userEvent.setup()
      await user.click(screen.getByRole('button', { name: 'Show password' }))
      expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'text')
      expect(screen.getByLabelText('Password')).toHaveValue('secret')
      expect(screen.getByRole('button', { name: 'Hide password' })).not.toHaveAttribute(
        'aria-pressed'
      )
      expect(submit).not.toHaveBeenCalled()
    })
    it('should disable both native input and password reveal together', async () => {
      render(<Field label="Password" type="password" disabled allowPasswordReveal />)
      expect(screen.getByLabelText('Password')).toBeDisabled()
      expect(screen.getByRole('button', { name: 'Show password' })).toBeDisabled()
      await userEvent.setup().click(screen.getByRole('button'))
      expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
    })
    it('should preserve read-only data, remove cleared errors, and retain caller validation state', () => {
      const { rerender } = render(
        <Field label="Name" readOnly defaultValue="Ada" error="Invalid" />
      )
      rerender(<Field label="Name" readOnly defaultValue="Ada" aria-invalid="grammar" />)
      expect(screen.queryByText('Invalid')).not.toBeInTheDocument()
      expect(screen.getByRole('textbox')).toHaveAttribute('readonly')
      expect(screen.getByRole('textbox')).toHaveAttribute('aria-invalid', 'grammar')
    })
    it('should render separate accessible inputs during SSR without browser APIs', () => {
      const html = renderToString(
        <>
          <Field label="First" />
          <Field label="Second" />
        </>
      )
      const container = document.createElement('div')
      container.innerHTML = html
      const ids = [...container.querySelectorAll('input')].map((input) => input.id)
      expect(ids).toHaveLength(2)
      expect(new Set(ids).size).toBe(2)
    })
  })
}

for (const Field of [InputExplorerNames, SharedWebField]) {
  it(`should reject void-element content safely for untyped ${Field.name} callers`, () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => {})
    try {
      expect(() =>
        render(
          <Field
            label="Email"
            {...({
              children: 'Unsupported',
              dangerouslySetInnerHTML: { __html: 'Unsupported' }
            } as object)}
          />
        )
      ).not.toThrow()
    } finally {
      error.mockRestore()
    }
  })
}

it('should expose caller-owned counters and suffixes as input descriptions', () => {
  render(<InputExplorerNames label="Name" defaultValue="Carolo" counter="6/15" suffix="#b47f" />)
  expect(screen.getByRole('textbox')).toHaveAccessibleDescription('#b47f 6/15')
  expect(screen.getByText('6/15')).toBeInTheDocument()
})

it('should omit blank counter and suffix descriptions without leaking custom native attributes', () => {
  render(<InputExplorerNames label="Name" counter=" " suffix=" " />)
  const input = screen.getByRole('textbox')
  expect(input).not.toHaveAttribute('aria-describedby')
  expect(input).not.toHaveAttribute('counter')
  expect(input).not.toHaveAttribute('suffix')
})

it('should keep captured error-vector references unique across repeated inputs', () => {
  const { container } = render(
    <>
      <InputExplorerNames label="First" error="Invalid" />
      <InputExplorerNames label="Second" error="Invalid" />
    </>
  )
  const ids = [...container.querySelectorAll('[id]')].map((node) => node.id)
  expect(new Set(ids).size).toBe(ids.length)
  for (const node of container.querySelectorAll('[mask]')) {
    const id = node.getAttribute('mask')!.slice(5, -1)
    expect(document.getElementById(id)).toBeInTheDocument()
  }
})

for (const Field of [InputExplorerNames, SharedWebField]) {
  it(`should restore masked defaults when an uncontrolled ${Field.name} form resets`, async () => {
    render(
      <form>
        <Field label="Password" type="password" defaultValue="secret" allowPasswordReveal />
        <button type="reset">Reset</button>
      </form>
    )
    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: 'Show password' }))
    await user.type(screen.getByLabelText('Password'), 'X')
    await user.click(screen.getByRole('button', { name: 'Reset' }))
    expect(screen.getByLabelText('Password')).toHaveValue('secret')
    expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
  })
  it(`should preserve ${Field.name} data and visibility when reset is canceled`, async () => {
    render(
      <form onReset={(event) => event.preventDefault()}>
        <Field label="Password" type="password" defaultValue="secret" allowPasswordReveal />
        <button type="reset">Reset</button>
      </form>
    )
    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: 'Show password' }))
    await user.type(screen.getByLabelText('Password'), 'X')
    await user.click(screen.getByRole('button', { name: 'Reset' }))
    expect(screen.getByLabelText('Password')).toHaveValue('secretX')
    expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'text')
  })
}

for (const Field of [InputExplorerNames, SharedWebField]) {
  it(`should expose the real ${Field.name} input through its public ref and clear it on unmount`, () => {
    const ref = createRef<HTMLInputElement>()
    const { unmount } = render(<Field label="Email" ref={ref} />)
    expect(ref.current).toBe(screen.getByRole('textbox'))
    ref.current!.focus()
    expect(screen.getByRole('textbox')).toHaveFocus()
    unmount()
    expect(ref.current).toBeNull()
  })
  it(`should reset ${Field.name} visibility through an external native form association`, async () => {
    render(
      <>
        <form id="external-form">
          <button type="reset">Reset</button>
        </form>
        <Field
          label="Password"
          form="external-form"
          type="password"
          defaultValue="secret"
          allowPasswordReveal
        />
      </>
    )
    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: 'Show password' }))
    await user.type(screen.getByLabelText('Password'), 'X')
    await user.click(screen.getByRole('button', { name: 'Reset' }))
    expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
    expect(screen.getByLabelText('Password')).toHaveValue('secret')
  })
}

it('should preserve a named hidden label, hint description and consumer-owned action', async () => {
  const action = vi.fn()
  render(
    <SharedWebField
      label="Email"
      labelHidden
      hint="Private address"
      hintIcon={<svg aria-hidden="true" />}
      action={
        <button type="button" onClick={action}>
          Verify
        </button>
      }
    />
  )
  expect(screen.getByRole('textbox', { name: 'Email' })).toHaveAccessibleDescription(
    'Private address'
  )
  expect(screen.getByText('Email')).toHaveAttribute('data-hidden', 'true')
  await userEvent.setup().click(screen.getByRole('button', { name: 'Verify' }))
  expect(action).toHaveBeenCalledOnce()
  expect(screen.getByRole('textbox')).toHaveValue('')
})
