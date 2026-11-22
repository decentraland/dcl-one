import { createRef, type FormEvent, type RefObject } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderToString } from 'react-dom/server'
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { InputExplorerNames } from '../InputExplorerNames/InputExplorerNames'
import { InputWebCreatorTools } from '../InputWebCreatorTools/InputWebCreatorTools'

afterEach(() => {
  vi.restoreAllMocks()
  vi.resetAllMocks()
})

for (const Field of [InputExplorerNames, InputWebCreatorTools]) {
  describe(`when ${Field.name} receives native input data`, () => {
    describe('and it has a label, description, error and a caller description', () => {
      let input: HTMLElement

      beforeEach(() => {
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
        input = screen.getByRole('textbox', { name: 'Email' })
      })

      it('should mark the input invalid', () => {
        expect(input).toHaveAttribute('aria-invalid', 'true')
      })

      it('should associate its description and error without removing the caller description', () => {
        expect(input).toHaveAccessibleDescription('External Use an address Invalid address')
      })

      it('should show the error without a live status', () => {
        expect(screen.getByText('Invalid address')).toBeInTheDocument()
        expect(screen.queryByRole('status')).toBeNull()
      })
    })

    describe('and it is controlled', () => {
      let change: Mock<(value: string) => void>

      beforeEach(async () => {
        change = vi.fn()
        render(
          <Field
            label="Name"
            value="Ada"
            name="name"
            onChange={(event) => change(event.currentTarget.value)}
          />
        )
        await userEvent.setup().type(screen.getByRole('textbox'), 'X')
      })

      it('should forward the change event', () => {
        expect(change).toHaveBeenCalledWith('AdaX')
      })

      it('should preserve the controlled value', () => {
        expect(screen.getByRole('textbox')).toHaveValue('Ada')
      })

      it('should preserve the native name', () => {
        expect(screen.getByRole('textbox')).toHaveAttribute('name', 'name')
      })
    })

    describe('and a password inside a form is revealed', () => {
      let submit: Mock<(event: FormEvent<HTMLFormElement>) => void>

      beforeEach(async () => {
        submit = vi.fn((event: FormEvent<HTMLFormElement>) => event.preventDefault())
        render(
          <form onSubmit={submit}>
            <Field label="Password" type="password" defaultValue="secret" allowPasswordReveal />
          </form>
        )
        await userEvent.setup().click(screen.getByRole('button', { name: 'Show password' }))
      })

      it('should reveal the password', () => {
        expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'text')
      })

      it('should keep the password value', () => {
        expect(screen.getByLabelText('Password')).toHaveValue('secret')
      })

      it('should name the action instead of exposing a pressed state', () => {
        expect(screen.getByRole('button', { name: 'Hide password' })).not.toHaveAttribute(
          'aria-pressed'
        )
      })

      it('should not submit the surrounding form', () => {
        expect(submit).not.toHaveBeenCalled()
      })
    })

    describe('and it is disabled with password reveal', () => {
      beforeEach(() => {
        render(<Field label="Password" type="password" disabled allowPasswordReveal />)
      })

      it('should disable both the native input and the password reveal together', () => {
        expect(screen.getByLabelText('Password')).toBeDisabled()
        expect(screen.getByRole('button', { name: 'Show password' })).toBeDisabled()
      })

      describe('and the reveal is clicked', () => {
        beforeEach(async () => {
          await userEvent.setup().click(screen.getByRole('button'))
        })

        it('should keep the password masked', () => {
          expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
        })
      })
    })

    describe('and a read-only field drops its error for a caller validation state', () => {
      beforeEach(() => {
        const { rerender } = render(
          <Field label="Name" readOnly defaultValue="Ada" error="Invalid" />
        )
        rerender(<Field label="Name" readOnly defaultValue="Ada" aria-invalid="grammar" />)
      })

      it('should remove the cleared error', () => {
        expect(screen.queryByText('Invalid')).not.toBeInTheDocument()
      })

      it('should preserve the read-only data', () => {
        expect(screen.getByRole('textbox')).toHaveAttribute('readonly')
      })

      it('should retain the caller validation state', () => {
        expect(screen.getByRole('textbox')).toHaveAttribute('aria-invalid', 'grammar')
      })
    })

    describe('and two fields are server-rendered', () => {
      let ids: string[]

      beforeEach(() => {
        const html = renderToString(
          <>
            <Field label="First" />
            <Field label="Second" />
          </>
        )
        const container = document.createElement('div')
        container.innerHTML = html
        ids = [...container.querySelectorAll('input')].map((input) => input.id)
      })

      it('should render both inputs without browser APIs', () => {
        expect(ids).toHaveLength(2)
      })

      it('should give each input its own id', () => {
        expect(new Set(ids).size).toBe(2)
      })
    })
  })
}

describe('when an untyped caller passes void-element content', () => {
  beforeEach(() => {
    vi.spyOn(console, 'error').mockImplementation(() => {})
  })

  for (const Field of [InputExplorerNames, InputWebCreatorTools]) {
    describe(`and the field is ${Field.name}`, () => {
      it('should reject the content safely', () => {
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
      })
    })
  }
})

describe('when InputExplorerNames receives caller-owned counters and suffixes', () => {
  beforeEach(() => {
    render(<InputExplorerNames label="Name" defaultValue="Carolo" counter="6/15" suffix="#b47f" />)
  })

  it('should expose them as input descriptions', () => {
    expect(screen.getByRole('textbox')).toHaveAccessibleDescription('#b47f 6/15')
  })

  it('should show the counter', () => {
    expect(screen.getByText('6/15')).toBeInTheDocument()
  })
})

describe('when InputExplorerNames receives blank counters and suffixes', () => {
  let input: HTMLElement

  beforeEach(() => {
    render(<InputExplorerNames label="Name" counter=" " suffix=" " />)
    input = screen.getByRole('textbox')
  })

  it('should omit the blank descriptions', () => {
    expect(input).not.toHaveAttribute('aria-describedby')
  })

  it('should not leak the custom props as native attributes', () => {
    expect(input).not.toHaveAttribute('counter')
    expect(input).not.toHaveAttribute('suffix')
  })
})

describe('when repeated inputs capture error-vector references', () => {
  let container: HTMLElement

  beforeEach(() => {
    ;({ container } = render(
      <>
        <InputExplorerNames label="First" error="Invalid" />
        <InputExplorerNames label="Second" error="Invalid" />
      </>
    ))
  })

  it('should keep every id unique', () => {
    const ids = [...container.querySelectorAll('[id]')].map((node) => node.id)
    expect(new Set(ids).size).toBe(ids.length)
  })

  it('should point every mask at an existing element', () => {
    for (const node of container.querySelectorAll('[mask]')) {
      const id = node.getAttribute('mask')!.slice(5, -1)
      expect(document.getElementById(id)).toBeInTheDocument()
    }
  })
})

for (const Field of [InputExplorerNames, InputWebCreatorTools]) {
  describe(`when an uncontrolled ${Field.name} form resets after a reveal and an edit`, () => {
    beforeEach(async () => {
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
    })

    it('should restore the default value', () => {
      expect(screen.getByLabelText('Password')).toHaveValue('secret')
    })

    it('should mask the password again', () => {
      expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
    })
  })

  describe(`when a ${Field.name} form reset is canceled after a reveal and an edit`, () => {
    beforeEach(async () => {
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
    })

    it('should preserve the edited value', () => {
      expect(screen.getByLabelText('Password')).toHaveValue('secretX')
    })

    it('should preserve the revealed visibility', () => {
      expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'text')
    })
  })
}

for (const Field of [InputExplorerNames, InputWebCreatorTools]) {
  describe(`when ${Field.name} receives a public ref`, () => {
    let ref: RefObject<HTMLInputElement | null>
    let unmount: () => void

    beforeEach(() => {
      ref = createRef<HTMLInputElement>()
      ;({ unmount } = render(<Field label="Email" ref={ref} />))
    })

    it('should expose the real input through the ref', () => {
      expect(ref.current).toBe(screen.getByRole('textbox'))
      ref.current!.focus()
      expect(screen.getByRole('textbox')).toHaveFocus()
    })

    describe('and it unmounts', () => {
      beforeEach(() => {
        unmount()
      })

      it('should clear the ref', () => {
        expect(ref.current).toBeNull()
      })
    })
  })

  describe(`when ${Field.name} belongs to an external native form that resets`, () => {
    beforeEach(async () => {
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
    })

    it('should mask the password again', () => {
      expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
    })

    it('should restore the default value', () => {
      expect(screen.getByLabelText('Password')).toHaveValue('secret')
    })
  })
}

describe('when InputWebCreatorTools has a hidden label, a hint and a consumer-owned action', () => {
  let action: Mock<() => void>

  beforeEach(() => {
    action = vi.fn()
    render(
      <InputWebCreatorTools
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
  })

  it('should describe the input with the hint', () => {
    expect(screen.getByRole('textbox', { name: 'Email' })).toHaveAccessibleDescription(
      'Private address'
    )
  })

  it('should keep the hidden label named', () => {
    expect(screen.getByText('Email')).toHaveAttribute('data-hidden', 'true')
  })

  describe('and the action is clicked', () => {
    beforeEach(async () => {
      await userEvent.setup().click(screen.getByRole('button', { name: 'Verify' }))
    })

    it('should run the consumer-owned action once', () => {
      expect(action).toHaveBeenCalledOnce()
    })

    it('should leave the input value alone', () => {
      expect(screen.getByRole('textbox')).toHaveValue('')
    })
  })
})
