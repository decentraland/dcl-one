import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

// The hosted pull request preview: `vite build --mode hosted` defines no Figma reference.
vi.stubGlobal('__GENERATED_COMPONENTS__', [])
vi.stubGlobal('__GENERATED_REVIEW_URL__', null)
const { Shell } = await import('../src/shell/Shell')

describe('when the gallery is the hosted preview without the Figma reference', () => {
  it('should answer a Figma reference link with a note pointing to the full build, not a frame', () => {
    render(<Shell search="?source=generated&component=Button" />)
    expect(screen.getByRole('note').textContent).toBe(
      'Not in this hosted preview: the full build linked from the pull request includes it.'
    )
    expect(screen.queryByTitle('Figma reference')).toBeNull()
    expect(screen.queryByRole('link', { name: 'Open it on its own' })).toBeNull()
  })

  it('should list no Figma reference entries in the sidebar', () => {
    render(<Shell search="?component=Button" />)
    const reference = screen
      .getByText('Figma reference', { selector: 'summary' })
      .closest('details')!
    expect(reference.querySelectorAll('a')).toHaveLength(0)
    expect(reference.textContent).toContain('Not in this hosted preview')
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Button')
  })
})
