// TextArea (design/TextArea): the shared InputExplorerNames in its HUD look, over several lines, with the kit's
// uncontrolled mode and its character counter.
import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { TextArea } from '../design'

describe('TextArea', () => {
  it('keeps its own text when uncontrolled, reports each change as a string and counts it', () => {
    const onChange = vi.fn()
    render(<TextArea aria-label="About me" defaultValue="hi" maxLength={10} counter onChange={onChange} />)
    const area = screen.getByRole('textbox', { name: 'About me' }) as HTMLTextAreaElement
    expect(area.tagName).toBe('TEXTAREA')
    expect(screen.getByText('2/10')).toBeTruthy()
    fireEvent.change(area, { target: { value: 'hello' } })
    expect(area.value).toBe('hello')
    expect(onChange).toHaveBeenCalledWith('hello')
    expect(screen.getByText('5/10')).toBeTruthy()
  })

  it('shows no counter without a maximum, or when it is not asked for', () => {
    const { rerender } = render(<TextArea aria-label="About me" defaultValue="hi" counter />)
    expect(screen.queryByText(/\d+\/\d+/)).toBeNull()
    rerender(<TextArea aria-label="About me" defaultValue="hi" maxLength={10} />)
    expect(screen.queryByText(/\d+\/\d+/)).toBeNull()
  })

  it('follows a controlled value, and passes its variant and rows', () => {
    const onChange = vi.fn()
    render(<TextArea aria-label="About me" value="hi" variant="light" rows={2} onChange={onChange} />)
    const area = screen.getByRole('textbox', { name: 'About me' }) as HTMLTextAreaElement
    fireEvent.change(area, { target: { value: 'hello' } })
    expect(onChange).toHaveBeenCalledWith('hello')
    expect(area.value).toBe('hi')
    expect(area.className).toMatch(/hudLight/)
    expect(area.getAttribute('rows')).toBe('2')
  })
})
