import { useState } from 'react'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderToString } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { Dropdown, type DropdownOption } from './Dropdown'

const SORTS: DropdownOption[] = [
  { value: 'newest', label: 'Newest' },
  { value: 'cheapest', label: 'Cheapest' },
  { value: 'most_expensive', label: 'Most expensive' },
  { value: 'name', label: 'Name' }
]
const RARITIES: DropdownOption[] = [
  { value: 'common', label: 'Common' },
  { value: 'rare', label: 'Rare' },
  { value: 'epic', label: 'Epic' }
]

function Single({
  onChange = () => {},
  start = 'newest',
  options = SORTS
}: {
  onChange?: (next: string) => void
  start?: string | null
  options?: DropdownOption[]
}) {
  const [value, setValue] = useState(start)
  return (
    <>
      <Dropdown
        label="Sort by"
        options={options}
        value={value}
        placeholder="Choose"
        onChange={(next) => {
          setValue(next)
          onChange(next)
        }}
      />
      <button type="button">Outside</button>
    </>
  )
}

function Multiple({ onChange = () => {} }: { onChange?: (next: string[]) => void }) {
  const [value, setValue] = useState<string[]>([])
  return (
    <Dropdown
      multiple
      label="Rarity"
      options={RARITIES}
      value={value}
      placeholder="All"
      onChange={(next) => {
        setValue(next)
        onChange(next)
      }}
    />
  )
}

const combobox = (name = 'Sort by') => screen.getByRole('combobox', { name })
const option = (name: string) => screen.getByRole('option', { name })
const active = () => document.getElementById(combobox().getAttribute('aria-activedescendant') ?? '')

describe('Dropdown', () => {
  describe('when rendered closed', () => {
    it('is a named combobox that shows the chosen option and controls a hidden listbox', () => {
      render(<Single />)
      const control = combobox()
      expect(control).toHaveTextContent('Newest')
      expect(control).toHaveAttribute('aria-expanded', 'false')
      expect(control).toHaveAttribute('aria-haspopup', 'listbox')
      expect(document.getElementById(control.getAttribute('aria-controls')!)).toHaveAttribute('role', 'listbox')
      expect(screen.queryByRole('listbox')).toBeNull()
      expect(screen.getByText('Sort by')).toBeVisible()
    })

    it('shows the placeholder while nothing is chosen', () => {
      render(<Single start={null} />)
      expect(combobox()).toHaveTextContent('Choose')
    })

    it('keeps the name when the label is visually hidden', () => {
      render(<Dropdown label="Sort by" labelHidden options={SORTS} value="name" onChange={() => {}} />)
      expect(combobox()).toHaveTextContent('Name')
    })

    it('renders on the server without a browser', () => {
      const html = renderToString(<Dropdown label="Sort by" options={SORTS} value="name" onChange={() => {}} />)
      expect(html).toContain('role="combobox"')
      expect(html).toContain('role="listbox"')
    })

    it('submits the choice with a form under its name', () => {
      render(
        <form data-testid="form">
          <Dropdown multiple name="rarity" label="Rarity" options={RARITIES} value={['rare', 'epic']} onChange={() => {}} />
        </form>
      )
      const data = new FormData(screen.getByTestId('form') as HTMLFormElement)
      expect(data.getAll('rarity')).toEqual(['rare', 'epic'])
    })
  })

  describe('when the reader uses a pointer', () => {
    it('opens on a press, chooses the pressed option and closes with the focus on the control', async () => {
      const user = userEvent.setup()
      const change = vi.fn()
      render(<Single onChange={change} />)
      await user.click(combobox())
      expect(combobox()).toHaveAttribute('aria-expanded', 'true')
      expect(within(screen.getByRole('listbox', { name: 'Sort by' })).getAllByRole('option')).toHaveLength(4)
      expect(option('Newest')).toHaveAttribute('aria-selected', 'true')
      await user.click(option('Cheapest'))
      expect(change).toHaveBeenCalledWith('cheapest')
      expect(combobox()).toHaveTextContent('Cheapest')
      expect(combobox()).toHaveAttribute('aria-expanded', 'false')
      expect(combobox()).toHaveFocus()
    })

    it('does not report the option that is already chosen', async () => {
      const user = userEvent.setup()
      const change = vi.fn()
      render(<Single onChange={change} />)
      await user.click(combobox())
      await user.click(option('Newest'))
      expect(change).not.toHaveBeenCalled()
      expect(combobox()).toHaveAttribute('aria-expanded', 'false')
    })

    it('closes on a second press of the control', async () => {
      const user = userEvent.setup()
      render(<Single />)
      await user.click(combobox())
      await user.click(combobox())
      expect(combobox()).toHaveAttribute('aria-expanded', 'false')
    })

    it('closes on a press outside, without a choice', async () => {
      const user = userEvent.setup()
      const change = vi.fn()
      render(<Single onChange={change} />)
      await user.click(combobox())
      await user.click(screen.getByRole('button', { name: 'Outside' }))
      expect(combobox()).toHaveAttribute('aria-expanded', 'false')
      expect(change).not.toHaveBeenCalled()
    })

    it('does not choose an option that is disabled', async () => {
      const user = userEvent.setup()
      const change = vi.fn()
      render(<Single onChange={change} options={[...SORTS, { value: 'rarest', label: 'Rarest', disabled: true }]} />)
      await user.click(combobox())
      expect(option('Rarest')).toHaveAttribute('aria-disabled', 'true')
      await user.click(option('Rarest'))
      expect(change).not.toHaveBeenCalled()
      expect(combobox()).toHaveAttribute('aria-expanded', 'true')
    })
  })

  describe('when the reader uses the keyboard', () => {
    it('opens with the arrow on the chosen option, moves with the arrows and chooses with Enter', async () => {
      const user = userEvent.setup()
      const change = vi.fn()
      render(<Single onChange={change} />)
      await user.tab()
      expect(combobox()).toHaveFocus()
      await user.keyboard('{ArrowDown}')
      expect(combobox()).toHaveAttribute('aria-expanded', 'true')
      expect(active()).toBe(option('Newest'))
      await user.keyboard('{ArrowDown}{ArrowDown}')
      expect(active()).toBe(option('Most expensive'))
      await user.keyboard('{ArrowUp}')
      expect(active()).toBe(option('Cheapest'))
      await user.keyboard('{Enter}')
      expect(change).toHaveBeenCalledWith('cheapest')
      expect(combobox()).toHaveAttribute('aria-expanded', 'false')
      expect(combobox()).toHaveFocus()
    })

    it('opens with Enter and with Space, and chooses with Space', async () => {
      const user = userEvent.setup()
      const change = vi.fn()
      render(<Single onChange={change} />)
      await user.tab()
      await user.keyboard('{Enter}')
      expect(combobox()).toHaveAttribute('aria-expanded', 'true')
      await user.keyboard('{Escape}')
      await user.keyboard(' ')
      expect(combobox()).toHaveAttribute('aria-expanded', 'true')
      await user.keyboard('{End} ')
      expect(change).toHaveBeenCalledWith('name')
    })

    it('goes to the first and last option with Home and End, and stops at the ends', async () => {
      const user = userEvent.setup()
      render(<Single />)
      await user.tab()
      await user.keyboard('{ArrowDown}{End}')
      expect(active()).toBe(option('Name'))
      await user.keyboard('{ArrowDown}')
      expect(active()).toBe(option('Name'))
      await user.keyboard('{Home}{ArrowUp}')
      expect(active()).toBe(option('Newest'))
    })

    it('passes over an option that is disabled', async () => {
      const user = userEvent.setup()
      render(
        <Single
          options={SORTS.map((entry) => (entry.value === 'cheapest' ? { ...entry, disabled: true } : entry))}
        />
      )
      await user.tab()
      await user.keyboard('{ArrowDown}{ArrowDown}')
      expect(active()).toBe(option('Most expensive'))
      await user.keyboard('{ArrowDown}{ArrowUp}{ArrowUp}')
      expect(active()).toBe(option('Newest'))
    })

    it('goes to the option that starts with the typed letters', async () => {
      const user = userEvent.setup()
      render(<Single />)
      await user.tab()
      await user.keyboard('m')
      expect(combobox()).toHaveAttribute('aria-expanded', 'true')
      expect(active()).toBe(option('Most expensive'))
      await user.keyboard('{End}n')
      expect(active()).toBe(option('Newest'))
      await user.keyboard('a')
      expect(active()).toBe(option('Name'))
    })

    it('cycles through repeated initials, skipping disabled options and wrapping', async () => {
      const user = userEvent.setup()
      render(<Single options={[SORTS[0]!, { value: 'none', label: 'None', disabled: true }, SORTS[3]!]} />)
      await user.tab()
      await user.keyboard('n')
      expect(active()).toBe(option('Newest'))
      await user.keyboard('n')
      expect(active()).toBe(option('Name'))
      await user.keyboard('n')
      expect(active()).toBe(option('Newest'))
    })

    it('includes a space in a multi-word search without committing the choice', async () => {
      const user = userEvent.setup()
      const change = vi.fn()
      render(<Single onChange={change} options={[{ value: 'ny', label: 'New York' }, { value: 'nd', label: 'New Delhi' }]} />)
      await user.tab()
      await user.keyboard('new d')
      expect(active()).toBe(option('New Delhi'))
      expect(change).not.toHaveBeenCalled()
      await user.keyboard('{Enter}')
      expect(change).toHaveBeenCalledWith('nd')
    })

    it('opens and dismisses with Alt+Arrow without committing the highlighted option', async () => {
      const user = userEvent.setup()
      const change = vi.fn()
      render(<Single onChange={change} />)
      await user.tab()
      await user.keyboard('{Alt>}{ArrowDown}{/Alt}{ArrowDown}')
      expect(active()).toBe(option('Cheapest'))
      await user.keyboard('{Alt>}{ArrowUp}{/Alt}')
      expect(combobox()).toHaveAttribute('aria-expanded', 'false')
      expect(combobox()).toHaveFocus()
      expect(change).not.toHaveBeenCalled()
    })

    it('supports keyboard input after a click without native pointer focus', async () => {
      const user = userEvent.setup()
      const change = vi.fn()
      render(<Single onChange={change} />)
      await user.click(screen.getByRole('button', { name: 'Outside' }))
      combobox().addEventListener('mousedown', event => event.preventDefault(), { once: true })
      await user.click(combobox())
      expect(combobox()).toHaveFocus()
      await user.keyboard('{ArrowDown}{Enter}')
      expect(change).toHaveBeenCalledWith('cheapest')
    })

    it('closes on Escape without a choice, keeps the focus and keeps the key from the page', async () => {
      const user = userEvent.setup()
      const change = vi.fn()
      const page = vi.fn()
      render(
        <div onKeyDown={page}>
          <Single onChange={change} />
        </div>
      )
      await user.tab()
      await user.keyboard('{ArrowDown}{ArrowDown}{Escape}')
      expect(combobox()).toHaveAttribute('aria-expanded', 'false')
      expect(combobox()).toHaveFocus()
      expect(change).not.toHaveBeenCalled()
      expect(page.mock.calls.some(([event]) => event.key === 'Escape')).toBe(false)
    })

    it('closes when Tab moves the focus on', async () => {
      const user = userEvent.setup()
      render(<Single />)
      await user.tab()
      await user.keyboard('{ArrowDown}')
      await user.tab()
      expect(screen.getByRole('button', { name: 'Outside' })).toHaveFocus()
      expect(combobox()).toHaveAttribute('aria-expanded', 'false')
    })
  })

  describe('when several options can be chosen', () => {
    it('marks the listbox, toggles each option and stays open', async () => {
      const user = userEvent.setup()
      const change = vi.fn()
      render(<Multiple onChange={change} />)
      const control = combobox('Rarity')
      expect(control).toHaveTextContent('All')
      await user.click(control)
      expect(screen.getByRole('listbox')).toHaveAttribute('aria-multiselectable', 'true')
      await user.click(option('Epic'))
      await user.click(option('Common'))
      expect(change).toHaveBeenLastCalledWith(['common', 'epic'])
      expect(control).toHaveAttribute('aria-expanded', 'true')
      expect(control).toHaveTextContent('Common, Epic')
      await user.click(option('Epic'))
      expect(change).toHaveBeenLastCalledWith(['common'])
      expect(option('Epic')).toHaveAttribute('aria-selected', 'false')
    })

    it('toggles with Space and Enter from the keyboard', async () => {
      const user = userEvent.setup()
      const change = vi.fn()
      render(<Multiple onChange={change} />)
      await user.tab()
      await user.keyboard('{ArrowDown} {ArrowDown}{Enter}')
      expect(change).toHaveBeenLastCalledWith(['common', 'rare'])
      expect(combobox('Rarity')).toHaveAttribute('aria-expanded', 'true')
    })

    it('shows the caller text for the chosen options', () => {
      render(
        <Dropdown
          multiple
          label="Rarity"
          options={RARITIES}
          value={['rare', 'epic']}
          valueText={(chosen) => `${chosen.length} chosen`}
          onChange={() => {}}
        />
      )
      expect(combobox('Rarity')).toHaveTextContent('2 chosen')
    })
  })

  describe('when disabled', () => {
    it('closes the list and omits its value from the form', async () => {
      const user = userEvent.setup()
      const field = (disabled: boolean) => (
        <form aria-label="Preferences">
          <Dropdown name="sort" label="Sort by" options={SORTS} value="newest" disabled={disabled} onChange={() => {}} />
        </form>
      )
      const { rerender } = render(field(false))
      await user.click(combobox())
      expect(screen.getByRole('listbox')).toBeVisible()
      rerender(field(true))
      expect(screen.queryByRole('listbox')).toBeNull()
      expect(new FormData(screen.getByRole<HTMLFormElement>('form')).has('sort')).toBe(false)
      rerender(field(false))
      expect(combobox()).toHaveAttribute('aria-expanded', 'false')
    })

    it('is a disabled native control that does not open', async () => {
      const user = userEvent.setup()
      render(<Dropdown label="Sort by" options={SORTS} value="newest" disabled onChange={() => {}} />)
      expect(combobox()).toBeDisabled()
      await user.click(combobox())
      expect(combobox()).toHaveAttribute('aria-expanded', 'false')
    })
  })

  describe('when it has help', () => {
    it('describes the control, and shows the text from the help button until Escape', async () => {
      const user = userEvent.setup()
      render(
        <Dropdown label="Sort by" help="Items not for sale have no price." options={SORTS} value="newest" disabled onChange={() => {}} />
      )
      expect(combobox()).toHaveAccessibleDescription('Items not for sale have no price.')
      const help = screen.getByRole('button', { name: 'More information' })
      expect(help).toHaveAttribute('aria-expanded', 'false')
      await user.click(help)
      expect(help).toHaveAttribute('aria-expanded', 'true')
      expect(document.getElementById(help.getAttribute('aria-controls')!)).toBeVisible()
      await user.keyboard('{Escape}')
      expect(help).toHaveAttribute('aria-expanded', 'false')
      expect(help).toHaveFocus()
    })
  })

  describe('when the focus leaves its open help', () => {
    it('closes the help text', async () => {
      const user = userEvent.setup()
      render(
        <>
          <Dropdown label="Sort by" help="Items not for sale have no price." options={SORTS} value="newest" onChange={() => {}} />
          <button type="button">Outside</button>
        </>
      )
      const help = screen.getByRole('button', { name: 'More information' })
      await user.click(help)
      expect(help).toHaveAttribute('aria-expanded', 'true')
      await user.tab()
      expect(combobox()).toHaveFocus()
      expect(help).toHaveAttribute('aria-expanded', 'false')
    })
  })

  describe('when the open list would pass the edge of the viewport', () => {
    it('moves the list back inside, a gutter from the edge', async () => {
      const user = userEvent.setup()
      const rect = (left: number, width: number) =>
        ({ left, right: left + width, top: 0, bottom: 40, width, height: 40, x: left, y: 0, toJSON: () => ({}) }) as DOMRect
      Object.defineProperty(document.documentElement, 'clientWidth', { configurable: true, value: 390 })
      Object.defineProperty(document.documentElement, 'clientHeight', { configurable: true, value: 800 })
      const measure = vi
        .spyOn(HTMLElement.prototype, 'getBoundingClientRect')
        .mockImplementation(function (this: HTMLElement) {
          return this.getAttribute('role') === 'listbox' ? rect(200, 344) : rect(200, 150)
        })
      render(<Single />)
      await user.click(combobox())
      // 200 + 344 passes 390 - 16 by 170, and the list can go no further left than the 16px gutter.
      expect(screen.getByRole('listbox').style.getPropertyValue('--dropdown-shift')).toBe('-170px')
      measure.mockRestore()
    })
  })
})
