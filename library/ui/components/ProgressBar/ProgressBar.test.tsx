import { render, screen } from '@testing-library/react'
import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { ProgressBar } from './ProgressBar'

describe('ProgressBar', () => {
  describe('when rendered with a value', () => {
    it('exposes a named progressbar with its range and value', () => {
      render(<ProgressBar aria-label="Upload" value={1} max={3} />)
      const bar = screen.getByRole('progressbar', { name: 'Upload' })
      expect(bar).toHaveAttribute('aria-valuemin', '0')
      expect(bar).toHaveAttribute('aria-valuemax', '3')
      expect(bar).toHaveAttribute('aria-valuenow', '1')
    })

    it('takes its name from another element', () => {
      render(
        <>
          <h2 id="title">Reward journey</h2>
          <ProgressBar aria-labelledby="title" value={40} />
        </>
      )
      expect(screen.getByRole('progressbar', { name: 'Reward journey' })).toHaveAttribute(
        'aria-valuemax',
        '100'
      )
    })

    it('shows the value text after the track and keeps the caller value text for assistive technology', () => {
      render(
        <ProgressBar aria-label="Steps" value={2} max={3} valueLabel="2/3" aria-valuetext="2 of 3 steps" />
      )
      expect(screen.getByText('2/3')).toBeVisible()
      expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuetext', '2 of 3 steps')
    })

    it('draws the share of the track the value is of its range', () => {
      render(<ProgressBar aria-label="Upload" value={1} max={4} />)
      expect(screen.getByRole('progressbar').style.getPropertyValue('--progress-bar-share')).toBe('25%')
    })

    it('draws the given share for a scale that is not linear, and still reports the value', () => {
      render(<ProgressBar aria-label="Journey" value={12} max={100} fraction={0.3} />)
      const bar = screen.getByRole('progressbar')
      expect(bar.style.getPropertyValue('--progress-bar-share')).toBe('30%')
      expect(bar).toHaveAttribute('aria-valuenow', '12')
    })

    it('renders on the server without a browser', () => {
      expect(renderToString(<ProgressBar aria-label="Upload" value={1} max={3} />)).toContain(
        'role="progressbar"'
      )
    })
  })

  describe('when the value is outside its range', () => {
    it.each([
      [-5, '0', '0%'],
      [250, '100', '100%'],
      [Number.NaN, '0', '0%']
    ])('reports and draws %s at the end of the range', (value, now, share) => {
      render(<ProgressBar aria-label="Upload" value={value} />)
      const bar = screen.getByRole('progressbar')
      expect(bar).toHaveAttribute('aria-valuenow', now)
      expect(bar.style.getPropertyValue('--progress-bar-share')).toBe(share)
    })

    it('reads a range that is not positive as 100', () => {
      render(<ProgressBar aria-label="Upload" value={50} max={0} />)
      expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuemax', '100')
    })
  })

  describe('when the bar is full', () => {
    it('shows the completed label in place of the value text', () => {
      render(
        <ProgressBar aria-label="Steps" value={3} max={3} valueLabel="3/3" completedLabel="Completed" />
      )
      expect(screen.getByText('Completed')).toBeVisible()
      expect(screen.queryByText('3/3')).toBeNull()
      expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '3')
    })

    it('keeps the track without a completed label', () => {
      render(<ProgressBar aria-label="Steps" value={3} max={3} valueLabel="3/3" />)
      expect(screen.getByText('3/3')).toBeVisible()
    })
  })

  describe('when the bar is not full', () => {
    it('does not show the completed label', () => {
      render(<ProgressBar aria-label="Steps" value={2} max={3} completedLabel="Completed" />)
      expect(screen.queryByText('Completed')).toBeNull()
    })
  })
})
