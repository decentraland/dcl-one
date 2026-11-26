// TextInput (design/TextInput): the shared InputExplorerNames in its HUD look, with the kit's uncontrolled mode,
// its string onChange and its icon action.
import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { TextInput } from '../design'

describe('TextInput', () => {
  it('keeps its own text when uncontrolled and reports each change as a string', () => {
    const onChange = vi.fn()
    render(<TextInput aria-label="Name" defaultValue="rob" onChange={onChange} />)
    const field = screen.getByRole('textbox', { name: 'Name' }) as HTMLInputElement
    expect(field.value).toBe('rob')
    fireEvent.change(field, { target: { value: 'robtfm' } })
    expect(field.value).toBe('robtfm')
    expect(onChange).toHaveBeenCalledWith('robtfm')
  })

  it('follows a controlled value and does not keep text of its own', () => {
    const onChange = vi.fn()
    render(<TextInput aria-label="Name" value="rob" onChange={onChange} />)
    const field = screen.getByRole('textbox', { name: 'Name' }) as HTMLInputElement
    fireEvent.change(field, { target: { value: 'robtfm' } })
    expect(onChange).toHaveBeenCalledWith('robtfm')
    expect(field.value).toBe('rob')
  })

  it('passes its variant, size and invalid state to the field', () => {
    render(<TextInput aria-label="Name" variant="light" size="lg" invalid />)
    const field = screen.getByRole('textbox', { name: 'Name' })
    expect(field.className).toMatch(/hudLight/)
    expect(field.className).toMatch(/hudLarge/)
    expect(field.getAttribute('aria-invalid')).toBe('true')
  })

  it('draws its action as a named button sized from the icon, and runs it', () => {
    const onClick = vi.fn()
    const { container } = render(<TextInput aria-label="Name" action={{ icon: 'dice', label: 'Pick another', onClick, size: 28 }} />)
    expect((container.firstElementChild as HTMLElement).style.getPropertyValue('--action-icon')).toBe('28px')
    fireEvent.click(screen.getByRole('button', { name: 'Pick another' }))
    expect(onClick).toHaveBeenCalledTimes(1)
  })
})
