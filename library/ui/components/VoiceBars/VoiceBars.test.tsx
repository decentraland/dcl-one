import '@testing-library/jest-dom/vitest'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { VoiceBars } from './VoiceBars'

describe('VoiceBars', () => {
  it('is an image named Speaking, with three bars', () => {
    render(<VoiceBars />)
    const badge = screen.getByRole('img', { name: 'Speaking' })
    expect(badge).toHaveAttribute('data-speaking-bars')
    expect(badge.children).toHaveLength(3)
  })

  it('takes the name it is given', () => {
    render(<VoiceBars label="Ada is speaking" />)
    expect(screen.getByRole('img', { name: 'Ada is speaking' })).toBeInTheDocument()
  })
})
