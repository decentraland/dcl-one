import '@testing-library/jest-dom/vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { Dropdown } from './Dropdown'

const options = [
  { value: 'low', label: 'Low' },
  { value: 'high', label: 'High' }
]

function Quality({ onChange = () => {}, start = 'low' }: { onChange?: (value: string) => void; start?: string }) {
  const [value, setValue] = useState(start)
  return (
    <>
      <Dropdown
        appearance="hud"
        label="Quality"
        value={value}
        options={options}
        onChange={(next) => {
          setValue(next)
          onChange(next)
        }}
      />
      <button type="button">Outside</button>
    </>
  )
}

describe('when a Dropdown takes the hud appearance', () => {
  it('should draw a named field with the chosen label and no list', () => {
    render(<Quality />)
    const field = screen.getByRole('button', { name: 'Quality' })
    expect(field).toHaveTextContent('Low')
    expect(field).toHaveAttribute('aria-haspopup', 'listbox')
    expect(field).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  it('should open its list of option buttons, mark the chosen one and choose another', async () => {
    const onChange = vi.fn()
    render(<Quality onChange={onChange} />)
    await userEvent.click(screen.getByRole('button', { name: 'Quality' }))
    expect(screen.getByRole('button', { name: 'Quality' })).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('option', { name: 'Low' })).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByRole('option', { name: 'High' })).toHaveAttribute('aria-selected', 'false')
    await userEvent.click(screen.getByRole('option', { name: 'High' }))
    expect(onChange).toHaveBeenCalledWith('high')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Quality' })).toHaveTextContent('High')
  })

  it('should close on a second click of the field and on a press outside', async () => {
    render(<Quality />)
    const field = screen.getByRole('button', { name: 'Quality' })
    await userEvent.click(field)
    await userEvent.click(field)
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
    await userEvent.click(field)
    await userEvent.click(screen.getByRole('button', { name: 'Outside' }))
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  it('should not open while disabled', async () => {
    render(<Dropdown appearance="hud" label="Quality" value="low" options={options} onChange={() => {}} disabled />)
    const field = screen.getByRole('button', { name: 'Quality' })
    expect(field).toBeDisabled()
    await userEvent.click(field)
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  it('should show the placeholder, then the raw value, when no option has the value', () => {
    const { rerender } = render(
      <Dropdown appearance="hud" label="Place" value="" options={options} onChange={() => {}} placeholder="Add a place" />
    )
    expect(screen.getByRole('button', { name: 'Place' })).toHaveTextContent('Add a place')
    rerender(<Dropdown appearance="hud" label="Place" value="custom" options={options} onChange={() => {}} />)
    expect(screen.getByRole('button', { name: 'Place' })).toHaveTextContent('custom')
  })

  it('should carry the light variant and its description', () => {
    render(
      <>
        <p id="hint">Lower it on a slow device.</p>
        <Dropdown
          appearance="hud"
          variant="light"
          label="Quality"
          aria-describedby="hint"
          value="low"
          options={options}
          onChange={() => {}}
        />
      </>
    )
    const field = screen.getByRole('button', { name: 'Quality' })
    expect(field.className).toMatch(/hudLight/)
    expect(field).toHaveAccessibleDescription('Lower it on a slow device.')
  })

  it('should open with no option when the list is empty', async () => {
    render(<Dropdown appearance="hud" label="Quality" value="" options={[]} onChange={() => {}} />)
    await userEvent.click(screen.getByRole('button', { name: 'Quality' }))
    expect(screen.getByRole('listbox')).toBeEmptyDOMElement()
  })
})
