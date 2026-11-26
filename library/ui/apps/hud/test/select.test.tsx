// Select (design/Select): the shared Dropdown in its HUD look, under the kit's props.
import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { Select } from '../design'

const options = [
  { value: 'low', label: 'Low' },
  { value: 'high', label: 'High' }
]

describe('Select', () => {
  it('names its field from aria-label, describes it and shows the chosen label', () => {
    render(
      <>
        <p id="hint">Lower it on a slow device.</p>
        <Select aria-label="Quality" aria-describedby="hint" value="high" options={options} onChange={() => {}} />
      </>
    )
    const field = screen.getByRole('button', { name: 'Quality' })
    expect(field.textContent).toBe('High')
    expect(field.getAttribute('aria-describedby')).toBe('hint')
  })

  it('passes its variant, its disabled state and its placeholder', () => {
    const { rerender } = render(<Select aria-label="Place" variant="light" value="" options={options} onChange={() => {}} placeholder="Add a place" />)
    const field = (): HTMLButtonElement => screen.getByRole('button', { name: 'Place' }) as HTMLButtonElement
    expect(field().className).toMatch(/hudLight/)
    expect(field().textContent).toBe('Add a place')
    expect(field().disabled).toBe(false)
    rerender(<Select aria-label="Place" value="low" options={options} onChange={() => {}} disabled />)
    expect(field().className).not.toMatch(/hudLight/)
    expect(field().disabled).toBe(true)
  })

  it('reports the value of the option picked', () => {
    const onChange = vi.fn()
    render(<Select aria-label="Quality" value="low" options={options} onChange={onChange} />)
    fireEvent.click(screen.getByRole('button', { name: 'Quality' }))
    fireEvent.click(screen.getByRole('option', { name: 'High' }))
    expect(onChange).toHaveBeenCalledWith('high')
    expect(screen.queryByRole('listbox')).toBeNull()
  })
})
