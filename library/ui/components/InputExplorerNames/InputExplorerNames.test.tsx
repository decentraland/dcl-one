import '@testing-library/jest-dom/vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createRef } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { InputExplorerNames } from './InputExplorerNames'

describe('when InputExplorerNames takes the hud appearance', () => {
  it('should render the input alone, named by its label and carrying the caller class', () => {
    const { container } = render(<InputExplorerNames appearance="hud" label="Name" className="mine" />)
    const field = screen.getByRole('textbox', { name: 'Name' })
    expect(container.firstElementChild).toBe(field)
    expect(field).toHaveClass('mine')
    expect(field.className).toMatch(/hudDark/)
    expect(field).not.toHaveAttribute('aria-invalid')
  })

  it("should leave the name to the caller's label element when it has no label", () => {
    render(
      <>
        <label htmlFor="alias">Alias</label>
        <InputExplorerNames appearance="hud" id="alias" />
      </>
    )
    const field = screen.getByRole('textbox', { name: 'Alias' })
    expect(field).not.toHaveAttribute('aria-label')
  })

  it('should prefer an aria-label the caller passes', () => {
    render(<InputExplorerNames appearance="hud" label="Name" aria-label="Find debug controls" />)
    expect(screen.getByRole('textbox', { name: 'Find debug controls' })).toBeInTheDocument()
  })

  it('should mark the light variant, the large size and an invalid value', () => {
    render(<InputExplorerNames appearance="hud" label="Name" variant="light" fieldSize="lg" invalid />)
    const field = screen.getByRole('textbox', { name: 'Name' })
    expect(field.className).toMatch(/hudLight/)
    expect(field.className).toMatch(/hudLarge/)
    expect(field.className).toMatch(/hudInvalid/)
    expect(field).toHaveAttribute('aria-invalid', 'true')
  })

  it('should pass native input props and its ref to the input', async () => {
    const ref = createRef<HTMLInputElement>()
    const onChange = vi.fn()
    render(<InputExplorerNames appearance="hud" label="Name" type="search" maxLength={3} ref={ref} onChange={onChange} />)
    const field = screen.getByRole('searchbox', { name: 'Name' })
    expect(ref.current).toBe(field)
    await userEvent.type(field, 'abcd')
    expect(field).toHaveValue('abc')
    expect(onChange).toHaveBeenCalledTimes(3)
  })

  describe('and it has an action', () => {
    it('should wrap the field, size the action from its icon and run it', async () => {
      const onClick = vi.fn()
      const { container } = render(
        <InputExplorerNames
          appearance="hud"
          label="Name"
          fieldAction={{ icon: <i data-testid="dice" />, label: 'Pick another', onClick, size: 28 }}
        />
      )
      const wrap = container.firstElementChild as HTMLElement
      expect(wrap.tagName).toBe('DIV')
      expect(wrap.style.getPropertyValue('--action-icon')).toBe('28px')
      expect(screen.getByRole('textbox', { name: 'Name' }).className).toMatch(/hudWithAction/)
      const action = screen.getByRole('button', { name: 'Pick another' })
      expect(action).toHaveAttribute('title', 'Pick another')
      expect(action).toContainElement(screen.getByTestId('dice'))
      await userEvent.click(action)
      expect(onClick).toHaveBeenCalledTimes(1)
    })

    it('should default the icon size to 18px', () => {
      const { container } = render(
        <InputExplorerNames appearance="hud" label="Name" fieldAction={{ icon: <i />, label: 'Pick', onClick: () => {} }} />
      )
      expect((container.firstElementChild as HTMLElement).style.getPropertyValue('--action-icon')).toBe('18px')
    })

    it('should hold the action while it or the field is disabled', async () => {
      const onClick = vi.fn()
      const { rerender } = render(
        <InputExplorerNames
          appearance="hud"
          label="Name"
          fieldAction={{ icon: <i />, label: 'Pick', onClick, disabled: true }}
        />
      )
      expect(screen.getByRole('button', { name: 'Pick' })).toBeDisabled()
      expect(screen.getByRole('textbox', { name: 'Name' })).toBeEnabled()
      rerender(
        <InputExplorerNames appearance="hud" label="Name" disabled fieldAction={{ icon: <i />, label: 'Pick', onClick }} />
      )
      expect(screen.getByRole('button', { name: 'Pick' })).toBeDisabled()
      await userEvent.click(screen.getByRole('button', { name: 'Pick' }))
      expect(onClick).not.toHaveBeenCalled()
    })
  })
})

describe('when InputExplorerNames names the default appearance', () => {
  it('should render the labelled Explorer field', () => {
    const { container } = render(<InputExplorerNames appearance="default" label="Name" />)
    expect(container.firstElementChild).toHaveAttribute('data-surface', 'explorer')
    expect(screen.getByRole('textbox', { name: 'Name' })).not.toHaveAttribute('appearance')
  })
})

describe('when the hud InputExplorerNames is multiline', () => {
  it('should render a textarea alone, four rows tall by default, with its variant and class', () => {
    const { container } = render(
      <InputExplorerNames appearance="hud" multiline label="About me" variant="light" className="mine" />
    )
    const area = screen.getByRole('textbox', { name: 'About me' })
    expect(area.tagName).toBe('TEXTAREA')
    expect(container.firstElementChild).toBe(area)
    expect(area).toHaveAttribute('rows', '4')
    expect(area.className).toMatch(/hudArea/)
    expect(area.className).toMatch(/hudLight/)
    expect(area).toHaveClass('mine')
    expect(area).not.toHaveAttribute('multiline')
  })

  it('should pass native textarea props and its ref', async () => {
    const ref = createRef<HTMLTextAreaElement>()
    const onChange = vi.fn()
    render(
      <InputExplorerNames appearance="hud" multiline label="About me" rows={2} maxLength={3} ref={ref} onChange={onChange} />
    )
    const area = screen.getByRole('textbox', { name: 'About me' })
    expect(ref.current).toBe(area)
    expect(area).toHaveAttribute('rows', '2')
    await userEvent.type(area, 'abcd')
    expect(area).toHaveValue('abc')
    expect(onChange).toHaveBeenCalledTimes(3)
  })

  it('should stay a single-line input when multiline is false', () => {
    render(<InputExplorerNames appearance="hud" multiline={false} label="Name" />)
    expect(screen.getByRole('textbox', { name: 'Name' }).tagName).toBe('INPUT')
  })
})
