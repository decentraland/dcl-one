import { render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { ExampleView } from '../src/frame/ExampleView'

afterEach(() => vi.restoreAllMocks())

const broken = {
  id: 'broken',
  title: 'Broken',
  render: (): never => {
    throw new Error('The example failed')
  }
}
const working = { id: 'working', title: 'Working', render: () => <p>Still drawn</p> }

describe('when an example fails while it renders', () => {
  it('should show the failure on its own stage and keep drawing the examples beside it', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {})
    const { container } = render(
      <>
        <ExampleView example={broken} />
        <ExampleView example={working} />
      </>
    )
    expect(screen.getByRole('alert').textContent).toBe('The example failed')
    expect(container.querySelector('[data-example="broken"] [data-example-error]')).not.toBeNull()
    expect(screen.getByText('Still drawn')).toBeTruthy()
  })
})
