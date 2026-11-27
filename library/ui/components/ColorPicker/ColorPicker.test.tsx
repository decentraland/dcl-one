import '@testing-library/jest-dom/vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { ColorPicker } from './ColorPicker'
import { hexToHsv, hsvToHex } from './hsv'

describe('the colour conversions', () => {
  it('round-trip a hex colour through hue, saturation and value', () => {
    for (const hex of ['#ffe4c6', '#20b3f6', '#1c1c1c', '#d4d4d4', '#8c2014', '#000000', '#ffffff'])
      expect(hsvToHex(hexToHsv(hex))).toBe(hex)
  })

  it('give a grey no hue and no saturation, and black no value', () => {
    expect(hexToHsv('#808080')).toMatchObject({ h: 0, s: 0 })
    expect(hexToHsv('#000000')).toEqual({ h: 0, s: 0, v: 0 })
  })

  it('read a colour without its hash, and each sixth of the hue wheel', () => {
    expect(hexToHsv('ff0000')).toEqual({ h: 0, s: 1, v: 1 })
    const wheel = ['#ff0000', '#ffff00', '#00ff00', '#00ffff', '#0000ff', '#ff00ff']
    wheel.forEach((hex, index) => {
      expect(hexToHsv(hex).h).toBeCloseTo(index * 60, 5)
      expect(hsvToHex({ h: index * 60, s: 1, v: 1 })).toBe(hex)
    })
  })
})

const presets = ['#1c1c1c', '#3c210b', '#ffbe28']

function open(value: string, props: Partial<Parameters<typeof ColorPicker>[0]> = {}) {
  const onChange = vi.fn()
  render(
    <ColorPicker label="Hair color" value={value} presets={presets} onChange={onChange} {...props} />
  )
  fireEvent.click(screen.getByRole('button', { name: props.label ?? 'Hair color' }))
  return onChange
}

describe('ColorPicker', () => {
  it('is a COLOR button until opened, and the button says whether the popup is open', () => {
    render(<ColorPicker label="Hair color" value="#3c210b" presets={presets} onChange={vi.fn()} />)
    const button = screen.getByRole('button', { name: 'Hair color' })
    expect(button).toHaveTextContent('COLOR')
    expect(button).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByRole('dialog')).toBeNull()
    fireEvent.click(button)
    expect(button).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('dialog', { name: 'Hair color' })).toBeInTheDocument()
    fireEvent.click(button)
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('draws range inputs and no native colour input', () => {
    open('#3c210b')
    expect(document.querySelector('input[type="color"]')).toBeNull()
    expect(screen.getAllByRole('slider').map((bar) => bar.getAttribute('type'))).toEqual([
      'range',
      'range',
      'range'
    ])
  })

  it('checks the preset that matches the colour, whatever its case, and picks another', () => {
    const onChange = open('#3C210B')
    expect(screen.getByRole('radio', { name: '#3c210b' })).toBeChecked()
    fireEvent.click(screen.getByRole('radio', { name: '#ffbe28' }))
    expect(onChange).toHaveBeenCalledWith('#ffbe28')
    expect(screen.getByRole('radio', { name: '#ffbe28' })).toBeChecked()
    expect(screen.getByRole('radio', { name: '#3c210b' })).not.toBeChecked()
  })

  it('checks no preset for a colour that is none of them', () => {
    open('#123456')
    expect(screen.getAllByRole('radio')).toHaveLength(3)
    for (const radio of screen.getAllByRole('radio')) expect(radio).not.toBeChecked()
  })

  it('draws no toggle without presets', () => {
    open('#123456', { presets: [] })
    expect(screen.getByRole('radiogroup', { name: 'Hair color presets' })).toBeEmptyDOMElement()
    expect(screen.getAllByRole('slider')).toHaveLength(3)
  })

  it('unchecks the preset when a bar moves', () => {
    open('#3c210b')
    fireEvent.change(screen.getByRole('slider', { name: 'Brightness' }), { target: { value: '0.5' } })
    expect(screen.getByRole('radio', { name: '#3c210b' })).not.toBeChecked()
  })

  it('steps a bar by a tenth with its arrows, and disables the arrow at each end', () => {
    const onChange = open('#ffbe28')
    expect(screen.getByRole('button', { name: 'Increase Brightness' })).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'Decrease Brightness' }))
    expect(hexToHsv(onChange.mock.calls.at(0)?.[0] as string).v).toBeCloseTo(0.9, 2)
    expect(screen.getByRole('button', { name: 'Increase Brightness' })).not.toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'Increase Brightness' }))
    expect(screen.getByRole('button', { name: 'Increase Brightness' })).toBeDisabled()
    fireEvent.change(screen.getByRole('slider', { name: 'Brightness' }), { target: { value: '0' } })
    expect(screen.getByRole('button', { name: 'Decrease Brightness' })).toBeDisabled()
    expect(onChange).toHaveBeenLastCalledWith('#000000')
  })

  it('stops a step at the end of the bar instead of passing it', () => {
    const onChange = open('#ffbe28')
    fireEvent.change(screen.getByRole('slider', { name: 'Saturation' }), { target: { value: '0.05' } })
    fireEvent.click(screen.getByRole('button', { name: 'Decrease Saturation' }))
    expect(hexToHsv(onChange.mock.calls.at(-1)?.[0] as string).s).toBe(0)
    expect(screen.getByRole('button', { name: 'Decrease Saturation' })).toBeDisabled()
    fireEvent.change(screen.getByRole('slider', { name: 'Hue' }), { target: { value: '0.95' } })
    fireEvent.click(screen.getByRole('button', { name: 'Increase Hue' }))
    expect(screen.getByRole('button', { name: 'Increase Hue' })).toBeDisabled()
  })

  it('keeps the hue when brightness goes to zero and back', () => {
    const onChange = open('#ffbe28')
    const brightness = screen.getByRole('slider', { name: 'Brightness' })
    fireEvent.change(brightness, { target: { value: '0' } })
    fireEvent.change(brightness, { target: { value: '1' } })
    expect(hexToHsv(onChange.mock.calls.at(-1)?.[0] as string).h).toBeCloseTo(
      hexToHsv('#ffbe28').h,
      0
    )
  })

  it('reads the colour again each time it opens', () => {
    const onChange = vi.fn()
    const view = render(
      <ColorPicker label="Hair color" value="#1c1c1c" presets={presets} onChange={onChange} />
    )
    const button = screen.getByRole('button', { name: 'Hair color' })
    fireEvent.click(button)
    expect(screen.getByRole('radio', { name: '#1c1c1c' })).toBeChecked()
    fireEvent.click(button)
    view.rerender(
      <ColorPicker label="Hair color" value="#ffbe28" presets={presets} onChange={onChange} />
    )
    fireEvent.click(button)
    expect(screen.getByRole('radio', { name: '#ffbe28' })).toBeChecked()
    expect(screen.getByRole('slider', { name: 'Brightness' })).toHaveValue('1')
  })

  it('closes on a press outside, and not on a press inside the popup', () => {
    open('#3c210b')
    fireEvent.mouseDown(screen.getByRole('slider', { name: 'Hue' }))
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    fireEvent.mouseDown(document.body)
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('does not close on Escape by itself', () => {
    open('#3c210b')
    fireEvent.keyDown(document.body, { key: 'Escape' })
    expect(screen.getByRole('dialog')).toBeInTheDocument()
  })

  it("hands the host a way to close the popup while it is open, and runs the host's cleanup as it leaves", () => {
    const off = vi.fn()
    let close: (() => void) | undefined
    const whileOpen = vi.fn((closer: () => void) => {
      close = closer
      return off
    })
    render(
      <ColorPicker
        label="Hair color"
        value="#3c210b"
        presets={presets}
        onChange={vi.fn()}
        whileOpen={whileOpen}
      />
    )
    expect(whileOpen).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Hair color' }))
    expect(whileOpen).toHaveBeenCalledTimes(1)
    expect(off).not.toHaveBeenCalled()
    act(() => close?.())
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(off).toHaveBeenCalledTimes(1)
  })

  it("runs the host's cleanup when it unmounts open", () => {
    const off = vi.fn()
    const view = render(
      <ColorPicker
        label="Hair color"
        value="#3c210b"
        presets={presets}
        onChange={vi.fn()}
        whileOpen={() => off}
      />
    )
    fireEvent.click(screen.getByRole('button', { name: 'Hair color' }))
    view.unmount()
    expect(off).toHaveBeenCalledTimes(1)
  })
})

describe('ColorPicker as a hue-only swatch', () => {
  const names = ['#a140ff', '#40eaff']
  const openSwatch = () => open('#40eaff', { label: 'Name color', presets: names, hueOnly: true, swatch: true })

  it('is the colour alone, not a COLOR button', () => {
    render(
      <ColorPicker label="Name color" value="#40eaff" presets={names} hueOnly swatch onChange={vi.fn()} />
    )
    const button = screen.getByRole('button', { name: 'Name color' })
    expect(button).not.toHaveTextContent('COLOR')
    expect(button.firstElementChild?.getAttribute('style')).toContain('rgb(64, 234, 255)')
  })

  it('offers the presets and the hue bar, without saturation or brightness', () => {
    openSwatch()
    expect(screen.getByRole('radio', { name: '#40eaff' })).toBeChecked()
    expect(screen.getByRole('slider', { name: 'Hue' })).toBeInTheDocument()
    expect(screen.queryByRole('slider', { name: 'Saturation' })).toBeNull()
    expect(screen.queryByRole('slider', { name: 'Brightness' })).toBeNull()
  })

  it('picks a hue at full saturation and brightness, even from a softer preset', () => {
    const onChange = openSwatch()
    fireEvent.change(screen.getByRole('slider', { name: 'Hue' }), { target: { value: '0' } })
    expect(onChange).toHaveBeenLastCalledWith('#ff0000')
    fireEvent.change(screen.getByRole('slider', { name: 'Hue' }), { target: { value: '1' } })
    expect(onChange).toHaveBeenLastCalledWith('#ff0000')
  })
})
