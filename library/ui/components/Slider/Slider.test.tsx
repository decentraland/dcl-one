import '@testing-library/jest-dom/vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { Slider } from './Slider'

describe('Slider', () => {
  describe('when it has no steppers', () => {
    it('is a native range input from 0 to 100 in steps of 1', () => {
      render(<Slider value={25} onChange={() => {}} aria-label="Volume" />)
      const slider = screen.getByRole('slider', { name: 'Volume' })
      expect(slider).toHaveAttribute('min', '0')
      expect(slider).toHaveAttribute('max', '100')
      expect(slider).toHaveAttribute('step', '1')
      expect(slider).toHaveValue('25')
      expect(slider).toHaveAttribute('data-variant', 'thin')
      expect(screen.queryByRole('button')).toBeNull()
    })

    it('reports the new value as a number', () => {
      const onChange = vi.fn()
      render(<Slider value={25} onChange={onChange} aria-label="Volume" />)
      fireEvent.change(screen.getByRole('slider'), { target: { value: '40' } })
      expect(onChange).toHaveBeenCalledWith(40)
    })

    it('fills the track up to its value', () => {
      render(<Slider value={6} min={0} max={24} onChange={() => {}} aria-label="Hour" />)
      const slider = screen.getByRole('slider')
      expect(slider.style.getPropertyValue('--pct')).toBe('25%')
      expect(slider.style.getPropertyValue('--frac')).toBe('0.25')
    })

    it('fills nothing when the range is empty', () => {
      render(<Slider value={5} min={5} max={5} onChange={() => {}} aria-label="Hour" />)
      expect(screen.getByRole('slider').style.getPropertyValue('--pct')).toBe('0%')
    })

    it('takes its variant, its description and the disabled state', () => {
      render(
        <Slider
          value={5}
          variant="rail"
          disabled
          onChange={() => {}}
          aria-label="Volume"
          aria-describedby="hint"
        />
      )
      const slider = screen.getByRole('slider')
      expect(slider).toHaveAttribute('data-variant', 'rail')
      expect(slider).toHaveAttribute('aria-describedby', 'hint')
      expect(slider).toBeDisabled()
    })
  })

  describe('when it has steppers', () => {
    it('moves one step down and one step up', () => {
      const onChange = vi.fn()
      render(<Slider arrows value={6} max={24} step={0.25} onChange={onChange} aria-label="Hour" />)
      fireEvent.click(screen.getByRole('button', { name: 'decrease' }))
      fireEvent.click(screen.getByRole('button', { name: 'increase' }))
      expect(onChange).toHaveBeenNthCalledWith(1, 5.75)
      expect(onChange).toHaveBeenNthCalledWith(2, 6.25)
    })

    it('holds the stepper that would leave the range', () => {
      const { rerender } = render(<Slider arrows value={0} onChange={() => {}} aria-label="Hour" />)
      expect(screen.getByRole('button', { name: 'decrease' })).toBeDisabled()
      expect(screen.getByRole('button', { name: 'increase' })).toBeEnabled()
      rerender(<Slider arrows value={100} onChange={() => {}} aria-label="Hour" />)
      expect(screen.getByRole('button', { name: 'decrease' })).toBeEnabled()
      expect(screen.getByRole('button', { name: 'increase' })).toBeDisabled()
    })

    it('clamps a step that would pass the end of the range', () => {
      const onChange = vi.fn()
      render(<Slider arrows value={98} step={5} onChange={onChange} aria-label="Hour" />)
      fireEvent.click(screen.getByRole('button', { name: 'increase' }))
      expect(onChange).toHaveBeenCalledWith(100)
    })

    it('names the steppers as the caller says', () => {
      render(
        <Slider
          arrows
          value={50}
          onChange={() => {}}
          aria-label="Volume"
          decreaseLabel="Lower the volume"
          increaseLabel="Raise the volume"
        />
      )
      expect(screen.getByRole('button', { name: 'Lower the volume' })).toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Raise the volume' })).toBeInTheDocument()
    })

    it('holds both steppers while disabled', () => {
      render(<Slider arrows value={50} disabled onChange={() => {}} aria-label="Hour" />)
      expect(screen.getByRole('button', { name: 'decrease' })).toBeDisabled()
      expect(screen.getByRole('button', { name: 'increase' })).toBeDisabled()
    })
  })
})
