import '@testing-library/jest-dom/vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { DateField } from './DateField'

// A date built from three HUD selects. `input type="date"` opens a native popup, which an offscreen host
// (the Explorer's CEF HUD) cannot composite: the picker replaced the whole HUD image there.

const pick = async (label: string, option: string): Promise<void> => {
  await userEvent.click(screen.getByRole('button', { name: label }))
  await userEvent.click(screen.getByRole('option', { name: option }))
}

describe('DateField', () => {
  it('shows the parts of the date it was given', () => {
    render(<DateField label="Birth Date" value="2003-02-01" onChange={vi.fn()} />)
    expect(screen.getByRole('button', { name: 'Birth Date day' })).toHaveTextContent('1')
    expect(screen.getByRole('button', { name: 'Birth Date month' })).toHaveTextContent('February')
    expect(screen.getByRole('button', { name: 'Birth Date year' })).toHaveTextContent('2003')
  })

  it('reports nothing until every part is chosen, then the whole date', async () => {
    const onChange = vi.fn()
    render(<DateField label="Birth Date" value="" onChange={onChange} />)

    await pick('Birth Date year', '2003')
    expect(onChange).toHaveBeenLastCalledWith('')
    await pick('Birth Date month', 'February')
    expect(onChange).toHaveBeenLastCalledWith('')
    await pick('Birth Date day', '1')
    expect(onChange).toHaveBeenLastCalledWith('2003-02-01')
  })

  it('never offers, or keeps, a day the month does not have', async () => {
    const onChange = vi.fn()
    render(<DateField label="Birth Date" value="2003-01-31" onChange={onChange} />)
    await pick('Birth Date month', 'February')
    // The 31st becomes the 28th rather than emitting a date that does not exist.
    expect(onChange).toHaveBeenLastCalledWith('2003-02-28')
    expect(screen.queryByRole('option', { name: '31' })).toBeNull()
  })

  it('offers the 29th in a leap year', async () => {
    render(<DateField label="Birth Date" value="2004-02-01" onChange={vi.fn()} />)
    await userEvent.click(screen.getByRole('button', { name: 'Birth Date day' }))
    expect(screen.getByRole('option', { name: '29' })).toBeTruthy()
  })

  it('clears back to nothing', async () => {
    const onChange = vi.fn()
    render(<DateField label="Birth Date" value="2003-02-01" onChange={onChange} />)
    await pick('Birth Date year', 'Year')
    expect(onChange).toHaveBeenLastCalledWith('')
  })

  it('draws three list buttons and no native date input', () => {
    const view = render(<DateField label="Birth Date" value="2003-02-01" onChange={vi.fn()} />)
    expect(screen.getAllByRole('button')).toHaveLength(3)
    expect(view.container.querySelector('input')).toBeNull()
    for (const button of screen.getAllByRole('button'))
      expect(button).toHaveAttribute('aria-haspopup', 'listbox')
  })

  it('shows the three prompts for an empty or a malformed date, and reports nothing by itself', () => {
    const onChange = vi.fn()
    const view = render(<DateField label="Birth Date" value="" onChange={onChange} />)
    const prompts = () => screen.getAllByRole('button').map((button) => button.textContent)
    expect(prompts()).toEqual(['Day', 'Month', 'Year'])
    for (const value of ['2003-2-1', 'yesterday', '2003-02-01T00:00:00Z']) {
      view.rerender(<DateField label="Birth Date" value={value} onChange={onChange} />)
      expect(prompts()).toEqual(['Day', 'Month', 'Year'])
    }
    expect(onChange).not.toHaveBeenCalled()
  })

  it('offers 31 days until a month is chosen', async () => {
    render(<DateField label="Birth Date" value="" onChange={vi.fn()} />)
    await userEvent.click(screen.getByRole('button', { name: 'Birth Date day' }))
    expect(screen.getAllByRole('option')).toHaveLength(32)
  })

  it('adopts a whole date arriving from outside, and keeps a chosen part when the value stays empty', async () => {
    const view = render(<DateField label="Birth Date" value="" onChange={vi.fn()} />)
    await pick('Birth Date month', 'March')
    view.rerender(<DateField label="Birth Date" value="" onChange={vi.fn()} />)
    expect(screen.getByRole('button', { name: 'Birth Date month' })).toHaveTextContent('March')
    view.rerender(<DateField label="Birth Date" value="1990-12-25" onChange={vi.fn()} />)
    expect(screen.getByRole('button', { name: 'Birth Date day' })).toHaveTextContent('25')
    expect(screen.getByRole('button', { name: 'Birth Date month' })).toHaveTextContent('December')
    expect(screen.getByRole('button', { name: 'Birth Date year' })).toHaveTextContent('1990')
  })

  it('offers the years from this one back to 1900', async () => {
    render(<DateField label="Birth Date" value="" onChange={vi.fn()} />)
    await userEvent.click(screen.getByRole('button', { name: 'Birth Date year' }))
    const years = screen.getAllByRole('option').map((option) => option.textContent)
    expect(years[1]).toBe(String(new Date().getUTCFullYear()))
    expect(years.at(-1)).toBe('1900')
  })

  it('opens nothing and reports nothing while disabled', async () => {
    const onChange = vi.fn()
    render(<DateField label="Birth Date" value="2003-02-01" onChange={onChange} disabled />)
    for (const button of screen.getAllByRole('button')) {
      expect(button).toBeDisabled()
      await userEvent.click(button)
    }
    expect(screen.queryByRole('option')).toBeNull()
    expect(onChange).not.toHaveBeenCalled()
  })
})
