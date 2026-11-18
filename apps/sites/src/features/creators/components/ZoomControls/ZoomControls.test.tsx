import { describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { type IPreviewController } from '@dcl/schemas'
import { TranslationProvider } from '~/intl'
import { ZOOM_STEP, ZoomControls } from './ZoomControls'

describe('ZoomControls', () => {
  it('nudges the camera in and out through the preview controller', async () => {
    const changeZoom = vi.fn().mockResolvedValue(undefined)
    const controller = { scene: { changeZoom } } as unknown as IPreviewController
    render(<ZoomControls controller={controller} />, { wrapper: TranslationProvider })
    await userEvent.click(screen.getByTestId('zoom-controls-in'))
    await userEvent.click(screen.getByTestId('zoom-controls-out'))
    expect(changeZoom.mock.calls).toEqual([[ZOOM_STEP], [-ZOOM_STEP]])
  })
})
