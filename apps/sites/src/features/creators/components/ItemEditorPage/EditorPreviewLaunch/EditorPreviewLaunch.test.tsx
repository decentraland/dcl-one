import { describe, it, expect, vi, beforeEach } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { EditorPreviewLaunch } from './EditorPreviewLaunch'

const state = vi.hoisted(() => ({ track: vi.fn(), previewCollection: vi.fn() }))
vi.mock('~/lib/analytics', () => ({ track: state.track }))
vi.mock('~/lib/explorer', () => ({ previewCollection: state.previewCollection }))

function renderLaunch(props: Partial<Parameters<typeof EditorPreviewLaunch>[0]> = {}) {
  return render(
    <TranslationProvider>
      <EditorPreviewLaunch collectionId="c1" itemCount={3} reviewing={false} {...props} />
    </TranslationProvider>
  )
}

beforeEach(() => {
  state.track.mockReset()
  state.previewCollection.mockReset().mockResolvedValue(true)
})

describe('EditorPreviewLaunch', () => {
  it('opens the collection in Decentraland for a curator and tracks it as a review launch', async () => {
    renderLaunch({ reviewing: true })
    fireEvent.click(screen.getByTestId('editor-preview'))
    expect(state.previewCollection).toHaveBeenCalledWith('c1')
    await waitFor(() =>
      expect(state.track).toHaveBeenCalledWith('Review Preview In World', {
        collectionId: 'c1',
        itemCount: 3,
        launched: true
      })
    )
  })

  it('tracks a creator launch from the editor', async () => {
    state.previewCollection.mockResolvedValue(false)
    renderLaunch()
    fireEvent.click(screen.getByTestId('editor-preview'))
    await waitFor(() =>
      expect(state.track).toHaveBeenCalledWith('Preview In World', {
        collectionId: 'c1',
        itemCount: 3,
        launched: false,
        trigger: 'editor'
      })
    )
  })

  it('shows it is busy while the launch settles', async () => {
    let settle!: (launched: boolean) => void
    state.previewCollection.mockReturnValue(new Promise<boolean>(resolve => (settle = resolve)))
    renderLaunch()
    fireEvent.click(screen.getByTestId('editor-preview'))
    expect(screen.getByTestId('editor-preview')).toHaveAttribute('aria-busy', 'true')
    settle(true)
    await waitFor(() => expect(screen.getByTestId('editor-preview')).not.toHaveAttribute('aria-busy'))
  })

  it('has nothing to preview in a collection without items', () => {
    renderLaunch({ reviewing: true, itemCount: 0 })
    expect(screen.getByTestId('editor-preview')).toBeDisabled()
  })
})
