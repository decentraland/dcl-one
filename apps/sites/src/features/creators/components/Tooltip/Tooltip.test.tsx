import { describe, it, expect } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Tooltip } from './Tooltip'

function renderTooltip() {
  return render(
    <Tooltip content="Helpful text">
      <span>i</span>
    </Tooltip>
  )
}

describe('Tooltip', () => {
  it('shows the content on hover and hides it when the pointer leaves', async () => {
    renderTooltip()
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument()

    await userEvent.hover(screen.getByTestId('tooltip-trigger'))
    expect(await screen.findByRole('tooltip')).toHaveTextContent('Helpful text')

    await userEvent.unhover(screen.getByTestId('tooltip-trigger'))
    await waitFor(() => expect(screen.queryByRole('tooltip')).not.toBeInTheDocument())
  })

  it('shows the content while the trigger has keyboard focus', async () => {
    renderTooltip()
    await userEvent.tab()
    expect(await screen.findByRole('tooltip')).toBeInTheDocument()
    await userEvent.tab()
    await waitFor(() => expect(screen.queryByRole('tooltip')).not.toBeInTheDocument())
  })
})

describe('Tooltip asChild', () => {
  it('uses the child element itself as the trigger', async () => {
    render(
      <Tooltip content="Blocked" asChild>
        <button data-testid="own-button">Publish</button>
      </Tooltip>
    )
    expect(screen.queryByTestId('tooltip-trigger')).not.toBeInTheDocument()
    await userEvent.hover(screen.getByTestId('own-button'))
    expect(await screen.findByRole('tooltip')).toHaveTextContent('Blocked')
  })

  it('renders the child alone when there is no content', async () => {
    render(
      <Tooltip content={null} asChild>
        <button data-testid="own-button">Publish</button>
      </Tooltip>
    )
    await userEvent.hover(screen.getByTestId('own-button'))
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument()
  })
})
