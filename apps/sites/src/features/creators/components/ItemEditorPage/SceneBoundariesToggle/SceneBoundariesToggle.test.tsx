import { beforeEach, describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { track } from '~/lib/analytics'
import { useAvatarPreview } from '~/store/avatarPreview'
import { SceneBoundariesToggle } from './SceneBoundariesToggle'

vi.mock('~/lib/analytics', () => ({ track: vi.fn() }))

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

beforeEach(() => {
  useAvatarPreview.setState({ showSceneBoundaries: false })
  vi.mocked(track).mockClear()
})

describe('SceneBoundariesToggle', () => {
  it('shows the boundaries around the avatar and hides them again', async () => {
    render(<SceneBoundariesToggle trigger="editor" />, { wrapper })
    const button = screen.getByTestId('scene-boundaries-toggle')
    expect(button).toHaveAttribute('aria-pressed', 'false')
    expect(button).toHaveAccessibleName('Show scene boundaries')

    await userEvent.click(button)
    expect(useAvatarPreview.getState().showSceneBoundaries).toBe(true)
    expect(button).toHaveAttribute('aria-pressed', 'true')
    expect(button).toHaveAccessibleName('Hide scene boundaries')
    expect(track).toHaveBeenCalledWith('Toggle scene boundaries', { visible: true, trigger: 'editor' })

    await userEvent.click(button)
    expect(useAvatarPreview.getState().showSceneBoundaries).toBe(false)
    expect(track).toHaveBeenLastCalledWith('Toggle scene boundaries', { visible: false, trigger: 'editor' })
  })
})
