import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { Tag } from './Tag'

describe('when a community supplies a role label', () => {
  it('shows the supplied role as text', () => {
    render(<Tag variant="role">Moderator</Tag>)
    expect(screen.getByText('Moderator')).toBeVisible()
  })

  it('keeps the role text in the emphasized appearance', () => {
    render(<Tag variant="role" appearance="emphasized">Owner</Tag>)
    expect(screen.getByText('Owner')).toHaveAttribute('data-appearance', 'emphasized')
  })
})
