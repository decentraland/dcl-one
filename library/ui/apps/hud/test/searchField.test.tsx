// SearchField (design/SearchField): the shared SearchBar in its HUD look, with the kit's uncontrolled mode,
// its Enter handler and its clear button.
import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { SearchField } from '../design'

describe('SearchField', () => {
  it('keeps its own text when uncontrolled, reports each change and clears', () => {
    const onChange = vi.fn()
    const onClear = vi.fn()
    render(<SearchField defaultValue="Gen" placeholder="Search places" clearable onChange={onChange} onClear={onClear} />)
    const field = screen.getByRole('textbox', { name: 'Search places' }) as HTMLInputElement
    expect(field.value).toBe('Gen')
    fireEvent.change(field, { target: { value: 'Genesis' } })
    expect(field.value).toBe('Genesis')
    expect(onChange).toHaveBeenLastCalledWith('Genesis')
    fireEvent.click(screen.getByRole('button', { name: 'Clear search' }))
    expect(field.value).toBe('')
    expect(onChange).toHaveBeenLastCalledWith('')
    expect(onClear).toHaveBeenCalledTimes(1)
    expect(screen.queryByRole('button', { name: 'Clear search' })).toBeNull()
  })

  it('shows the clear button on a light field without `clearable`, and none on a dark one', () => {
    const { rerender } = render(<SearchField variant="light" defaultValue="Gen" />)
    expect(screen.getByRole('button', { name: 'Clear search' })).toBeTruthy()
    rerender(<SearchField key="dark" defaultValue="Gen" />)
    expect(screen.queryByRole('button', { name: 'Clear search' })).toBeNull()
  })

  it('follows a controlled value and does not keep text of its own', () => {
    const onChange = vi.fn()
    render(<SearchField value="Gen" onChange={onChange} />)
    const field = screen.getByRole('textbox', { name: 'Search' }) as HTMLInputElement
    fireEvent.change(field, { target: { value: 'Genesis' } })
    expect(onChange).toHaveBeenCalledWith('Genesis')
    expect(field.value).toBe('Gen')
  })

  it('reports Enter with the text, and ignores other keys', () => {
    const onSubmit = vi.fn()
    render(<SearchField defaultValue="plaza" onSubmit={onSubmit} />)
    const field = screen.getByRole('textbox', { name: 'Search' })
    fireEvent.keyDown(field, { key: 'a' })
    expect(onSubmit).not.toHaveBeenCalled()
    fireEvent.keyDown(field, { key: 'Enter' })
    expect(onSubmit).toHaveBeenCalledWith('plaza')
  })

  it('passes its variant and size to the field', () => {
    const { rerender } = render(<SearchField variant="light" size="sm" />)
    const root = (): HTMLElement => screen.getByRole('textbox', { name: 'Search' }).parentElement!
    expect(root().getAttribute('data-variant')).toBe('light')
    expect(root().getAttribute('data-size')).toBe('sm')
    rerender(<SearchField />)
    expect(root().getAttribute('data-variant')).toBe('dark')
    expect(root().getAttribute('data-size')).toBe('md')
  })
})
