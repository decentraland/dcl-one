import { createRef, type RefObject, useState } from 'react'
import { render, type RenderResult, screen } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { flushSync } from 'react-dom'
import { renderToString } from 'react-dom/server'
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  type Mock,
  type MockInstance,
  vi
} from 'vitest'
import { Checkbox } from './Checkbox'

function SelectAll() {
  const [state, setState] = useState<'mixed' | boolean>('mixed')
  return (
    <Checkbox
      aria-label="Select all"
      checked={state === true}
      indeterminate={state === 'mixed'}
      onCheckedChange={setState}
    />
  )
}

function Stubborn() {
  const [, force] = useState(0)
  return (
    <Checkbox
      aria-label="Stubborn"
      checked={false}
      indeterminate
      onCheckedChange={() => force((n) => n + 1)}
    />
  )
}

function SyncSelectAll() {
  const [state, setState] = useState<'mixed' | boolean>('mixed')
  return (
    <Checkbox
      aria-label="Sync select all"
      checked={state === true}
      indeterminate={state === 'mixed'}
      onCheckedChange={(next) => flushSync(() => setState(next))}
    />
  )
}

describe('Checkbox', () => {
  let user: UserEvent

  beforeEach(() => {
    user = userEvent.setup()
  })

  afterEach(() => {
    vi.restoreAllMocks()
    vi.resetAllMocks()
  })

  describe('when rendered with a name', () => {
    describe('and no size or variant', () => {
      let container: HTMLElement

      beforeEach(() => {
        container = render(<Checkbox aria-label="Accept terms" />).container
      })

      it('should expose a named, unchecked checkbox with outline and md as defaults', () => {
        const box = screen.getByRole('checkbox', { name: 'Accept terms' })
        expect(box).not.toBeChecked()
        const root = container.firstElementChild!
        expect(root).toHaveAttribute('data-size', 'md')
        expect(root).toHaveAttribute('data-variant', 'outline')
      })
    })

    describe('and the name comes from aria-labelledby', () => {
      beforeEach(() => {
        render(
          <>
            <span id="caption">Remember me</span>
            <Checkbox aria-labelledby="caption" />
          </>
        )
      })

      it('should take its name from the visible text', () => {
        expect(screen.getByRole('checkbox', { name: 'Remember me' })).toBeInTheDocument()
      })
    })

    describe('and the name is missing', () => {
      beforeEach(() => {
        // @ts-expect-error: a checkbox needs aria-label or aria-labelledby
        render(<Checkbox />)
      })

      it('should refuse the checkbox at the type level and render it without a name', () => {
        expect(screen.getByRole('checkbox')).not.toHaveAccessibleName()
      })
    })

    describe.each(['sm', 'md', 'lg'] as const)('and the size is %s', (size) => {
      let container: HTMLElement

      beforeEach(() => {
        container = render(<Checkbox aria-label="Size" size={size} />).container
      })

      it(`should render the ${size} size`, () => {
        expect(container.firstElementChild).toHaveAttribute('data-size', size)
      })
    })

    describe('and the variant is filled', () => {
      let container: HTMLElement

      beforeEach(() => {
        container = render(<Checkbox aria-label="Filled" variant="filled" />).container
      })

      it('should render the filled variant', () => {
        expect(container.firstElementChild).toHaveAttribute('data-variant', 'filled')
      })
    })

    describe('and it is checked', () => {
      let container: HTMLElement

      beforeEach(() => {
        container = render(<Checkbox aria-label="Hidden glyphs" defaultChecked />).container
      })

      it('should hide the box and glyphs from assistive technology', () => {
        for (const svg of container.querySelectorAll('svg')) {
          expect(svg.closest('[aria-hidden="true"]')).not.toBeNull()
        }
      })
    })

    describe('and given a ref', () => {
      let ref: RefObject<HTMLInputElement | null>

      beforeEach(() => {
        ref = createRef<HTMLInputElement>()
        render(<Checkbox aria-label="Ref" ref={ref} />)
      })

      it('should forward its ref to the native input', () => {
        expect(ref.current).toBe(screen.getByRole('checkbox'))
      })
    })

    describe('and rendered on the server', () => {
      let html: string

      beforeEach(() => {
        html = renderToString(<Checkbox aria-label="Server" defaultChecked />)
      })

      it('should render without a browser', () => {
        expect(html).toContain('type="checkbox"')
        expect(html).toContain('aria-label="Server"')
      })
    })
  })

  describe('when the reader toggles it', () => {
    describe('and toggles by click and by Space', () => {
      let change: Mock
      let checked: Mock

      beforeEach(() => {
        change = vi.fn()
        checked = vi.fn()
        render(<Checkbox aria-label="Newsletter" onChange={change} onCheckedChange={checked} />)
      })

      it('should report the next state, keeping the native change event', async () => {
        const box = screen.getByRole('checkbox')
        await user.click(box)
        expect(checked).toHaveBeenLastCalledWith(true)
        expect(box).toBeChecked()
        await user.keyboard(' ')
        expect(checked).toHaveBeenLastCalledWith(false)
        expect(change).toHaveBeenCalledTimes(2)
      })
    })

    describe('and moves with Tab', () => {
      beforeEach(() => {
        render(
          <>
            <button type="button">Before</button>
            <Checkbox aria-label="Focusable" />
          </>
        )
      })

      it('should be reached by Tab and show focus on the native input', async () => {
        await user.tab()
        await user.tab()
        expect(screen.getByRole('checkbox')).toHaveFocus()
      })
    })

    describe('and presses Enter', () => {
      let checked: Mock

      beforeEach(() => {
        checked = vi.fn()
        render(<Checkbox aria-label="Enter" onCheckedChange={checked} />)
      })

      it('should ignore Enter, as a native checkbox does', async () => {
        screen.getByRole('checkbox').focus()
        await user.keyboard('{Enter}')
        expect(checked).not.toHaveBeenCalled()
      })
    })

    describe('and React reports console errors', () => {
      let error: MockInstance<typeof console.error>

      beforeEach(() => {
        error = vi.spyOn(console, 'error').mockImplementation(() => {})
      })

      describe('and the box is checked with no way to change it', () => {
        beforeEach(() => {
          render(<Checkbox aria-label="Frozen" checked />)
        })

        it('should let React warn about the missing change handler', () => {
          expect(error.mock.calls.flat().join(' ')).toMatch(/without an `onChange` handler/)
        })
      })

      describe('and the caller handles the change', () => {
        beforeEach(() => {
          render(<Checkbox aria-label="Handled" checked onCheckedChange={() => {}} />)
        })

        it('should not warn', () => {
          expect(error).not.toHaveBeenCalled()
        })
      })
    })

    describe('and the box is controlled', () => {
      beforeEach(() => {
        render(<Checkbox aria-label="Controlled" checked={false} onCheckedChange={() => {}} />)
      })

      it('should stay as the caller set it', async () => {
        await user.click(screen.getByRole('checkbox'))
        expect(screen.getByRole('checkbox')).not.toBeChecked()
      })
    })
  })

  describe('when indeterminate', () => {
    describe('and uncontrolled', () => {
      beforeEach(() => {
        render(<Checkbox aria-label="Select all" indeterminate />)
      })

      it('should report the mixed state on the native input', () => {
        const box = screen.getByRole<HTMLInputElement>('checkbox')
        expect(box.indeterminate).toBe(true)
        expect(box).toBePartiallyChecked()
      })
    })

    describe('and a controlled caller settles the next state', () => {
      beforeEach(() => {
        render(<SelectAll />)
      })

      it('should hand the click to the caller', async () => {
        const box = screen.getByRole<HTMLInputElement>('checkbox')
        await user.click(box)
        expect(box.indeterminate).toBe(false)
        expect(box).toBeChecked()
      })
    })

    describe('and the caller does not accept the click', () => {
      beforeEach(() => {
        render(<Stubborn />)
      })

      it('should keep the caller’s mixed state', async () => {
        const box = screen.getByRole<HTMLInputElement>('checkbox')
        await user.click(box)
        expect(box.indeterminate).toBe(true)
        expect(box).not.toBeChecked()
      })
    })

    describe('and the caller commits synchronously from its handler', () => {
      beforeEach(() => {
        render(<SyncSelectAll />)
      })

      it('should take the committed state', async () => {
        const box = screen.getByRole<HTMLInputElement>('checkbox')
        await user.click(box)
        expect(box.indeterminate).toBe(false)
        expect(box).toBeChecked()
      })
    })

    describe('and the caller keeps it mixed without rendering', () => {
      let checked: Mock

      beforeEach(() => {
        checked = vi.fn()
        render(<Checkbox aria-label="Kept" indeterminate onCheckedChange={checked} />)
      })

      it('should stay mixed', async () => {
        const box = screen.getByRole<HTMLInputElement>('checkbox')
        await user.click(box)
        expect(checked).toHaveBeenCalledWith(true)
        expect(box.indeterminate).toBe(true)
        expect(box).toBePartiallyChecked()
      })
    })

    describe('and the caller clears it', () => {
      let rendered: RenderResult

      beforeEach(() => {
        rendered = render(<Checkbox aria-label="Clear" indeterminate />)
      })

      it('should drop the mixed state', () => {
        rendered.rerender(<Checkbox aria-label="Clear" indeterminate={false} />)
        expect(screen.getByRole<HTMLInputElement>('checkbox').indeterminate).toBe(false)
      })
    })
  })

  describe('when disabled', () => {
    let checked: Mock

    beforeEach(() => {
      checked = vi.fn()
      render(<Checkbox aria-label="Locked" disabled onCheckedChange={checked} />)
    })

    it('should be disabled and ignore clicks and keys', async () => {
      const box = screen.getByRole('checkbox', { name: 'Locked' })
      expect(box).toBeDisabled()
      await user.click(box)
      box.focus()
      await user.keyboard(' ')
      expect(checked).not.toHaveBeenCalled()
      expect(box).not.toBeChecked()
      expect(box).not.toHaveFocus()
    })
  })

  describe('when inside a form', () => {
    describe('and the form is read', () => {
      beforeEach(() => {
        render(
          <form aria-label="preferences">
            <Checkbox aria-label="Events" name="events" defaultChecked />
            <Checkbox aria-label="Tips" name="tips" />
            <Checkbox aria-label="Mixed" name="mixed" indeterminate />
          </form>
        )
      })

      it('should submit its name and value only while checked', () => {
        const data = new FormData(screen.getByRole('form'))
        expect(data.get('events')).toBe('on')
        expect(data.has('tips')).toBe(false)
        expect(data.has('mixed')).toBe(false)
      })
    })

    describe('and the form is reset', () => {
      beforeEach(() => {
        render(
          <form aria-label="reset">
            <Checkbox aria-label="Reset me" name="reset" />
            <button type="reset">Reset</button>
          </form>
        )
      })

      it('should return to its default state', async () => {
        const box = screen.getByRole('checkbox')
        await user.click(box)
        expect(box).toBeChecked()
        await user.click(screen.getByRole('button', { name: 'Reset' }))
        expect(box).not.toBeChecked()
      })
    })
  })
})
