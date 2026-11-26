import { describe, it, expect, vi } from 'vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import { ColorPicker } from '../design'
import { dispatchCancelLayer } from '@explorer-core/lib/cancelLayers'
import { color3ToHex, hexToColor3 } from '@explorer-core/lib/color'

describe('color conversions', () => {
  it('maps engine Color3 (0–1) to hex and back', () => {
    expect(color3ToHex({ r: 1, g: 0.5, b: 0 })).toBe('#ff8000')
    expect(hexToColor3('#ff8000')).toEqual({ r: 1, g: 128 / 255, b: 0 })
  })

  it('clamps a channel outside 0–1', () => {
    expect(color3ToHex({ r: 2, g: -1, b: 0.5 })).toBe('#ff0080')
  })
})

// The HUD's ColorPicker is the shared one (library/ui/components/ColorPicker, tested there) registered as
// a cancel layer while its popup is open.
describe('ColorPicker', () => {
  const presets = ['#1c1c1c', '#3c210b', '#ffbe28']
  const open = (value: string, onChange = vi.fn()) => {
    render(<ColorPicker label="Hair color" value={value} presets={presets} onChange={onChange} />)
    fireEvent.click(screen.getByRole('button', { name: 'Hair color' }))
    return onChange
  }

  it('is a COLOR button until opened, and no cancel layer', () => {
    render(<ColorPicker label="Hair color" value="#3c210b" presets={presets} onChange={vi.fn()} />)
    expect(screen.getByRole('button', { name: 'Hair color' })).toHaveTextContent('COLOR')
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(dispatchCancelLayer()).toBe(false)
  })

  it('highlights the preset matching the current colour and picks another', () => {
    const onChange = open('#3c210b')
    expect(screen.getByRole('radio', { name: '#3c210b' })).toBeChecked()
    fireEvent.click(screen.getByRole('radio', { name: '#ffbe28' }))
    expect(onChange).toHaveBeenCalledWith('#ffbe28')
    expect(screen.getByRole('radio', { name: '#ffbe28' })).toBeChecked()
    fireEvent.mouseDown(document.body)
  })

  it('Cancel closes just the popup', () => {
    open('#3c210b')
    act(() => { expect(dispatchCancelLayer()).toBe(true) })
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(dispatchCancelLayer()).toBe(false)
  })

  it('a click outside closes the popup and leaves no cancel layer', () => {
    open('#3c210b')
    fireEvent.mouseDown(document.body)
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(dispatchCancelLayer()).toBe(false)
  })

  it('unmounting it open leaves no cancel layer', () => {
    const view = render(<ColorPicker label="Hair color" value="#3c210b" presets={presets} onChange={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: 'Hair color' }))
    view.unmount()
    expect(dispatchCancelLayer()).toBe(false)
  })

  it('the name colour form is a swatch with the hue bar only', () => {
    const onChange = vi.fn()
    render(<ColorPicker label="Name color" value="#40eaff" presets={['#a140ff', '#40eaff']} hueOnly swatch onChange={onChange} />)
    const button = screen.getByRole('button', { name: 'Name color' })
    expect(button).not.toHaveTextContent('COLOR')
    fireEvent.click(button)
    expect(screen.queryByRole('slider', { name: 'Saturation' })).toBeNull()
    fireEvent.change(screen.getByRole('slider', { name: 'Hue' }), { target: { value: '0' } })
    expect(onChange).toHaveBeenLastCalledWith('#ff0000')
    fireEvent.mouseDown(document.body)
  })
})
