import { afterEach, describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { Confetti } from './Confetti'

function stubReducedMotion(matches: boolean) {
  vi.stubGlobal('matchMedia', vi.fn().mockReturnValue({ matches }))
}

afterEach(() => vi.unstubAllGlobals())

describe('Confetti', () => {
  it('fires the burst layer for a viewer who has not asked for less motion', () => {
    stubReducedMotion(false)
    render(<Confetti />)
    expect(screen.getByTestId('confetti')).toBeInTheDocument()
  })

  it('stays silent when the viewer prefers reduced motion', () => {
    stubReducedMotion(true)
    render(<Confetti />)
    expect(screen.queryByTestId('confetti')).not.toBeInTheDocument()
  })
})
