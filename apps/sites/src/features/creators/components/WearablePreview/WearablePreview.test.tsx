import { describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { BodyShape } from '@dcl/schemas'
import { type WearablePreviewController } from './types'

const controlProps: Record<string, Record<string, unknown>> = {}
vi.mock('decentraland-ui2/dist/components/WearablePreview/EmoteControls', () => ({
  EmoteControls: (props: Record<string, unknown>) => {
    controlProps.emote = props
    return <div data-testid="emote-controls" />
  }
}))
vi.mock('decentraland-ui2/dist/components/WearablePreview/TranslationControls', async importOriginal => ({
  ...(await importOriginal<object>()),
  TranslationControls: (props: Record<string, unknown>) => {
    controlProps.translation = props
    return <div data-testid="translation-controls" />
  }
}))

// MUI's CssVarsProvider follows the system color scheme, which jsdom has no matchMedia for.
vi.stubGlobal('matchMedia', () => ({
  matches: false,
  addListener: () => undefined,
  removeListener: () => undefined,
  addEventListener: () => undefined,
  removeEventListener: () => undefined
}))

const { WearablePreview, createWearablePreviewController, EmoteControls, TranslationControls } =
  await import('./WearablePreview')

describe('WearablePreview', () => {
  it('mounts an iframe under the given id with the options baked into its URL', () => {
    render(<WearablePreview id="preview" bodyShape={BodyShape.FEMALE} skin="aabbcc" unity />)
    const iframe = document.getElementById('preview')
    expect(iframe).toBeInstanceOf(HTMLIFrameElement)
    const params = new URL((iframe as HTMLIFrameElement).src).searchParams
    expect(params.get('skin')).toBe('aabbcc')
    expect(params.get('bodyShape')).toBe(BodyShape.FEMALE)
    expect(params.get('unity')).toBe('true')
  })

  it('creates a controller for the mounted preview', () => {
    render(<WearablePreview id="preview" />)
    const controller = createWearablePreviewController('preview')
    expect(controller.emote.events.on).toBeTypeOf('function')
    expect(controller.scene.getScreenshot).toBeTypeOf('function')
  })

  it('points the controls at the preview and the shared controller', async () => {
    const controller = {} as WearablePreviewController
    render(
      <>
        <EmoteControls previewId="preview" controller={controller} hideSoundButton />
        <TranslationControls previewId="preview" controller={controller} vertical verticalPosition="left" />
      </>
    )
    await screen.findByTestId('emote-controls')
    await screen.findByTestId('translation-controls')
    expect(controlProps.emote).toMatchObject({
      wearablePreviewId: 'preview',
      wearablePreviewController: controller,
      hideSoundButton: true
    })
    expect(controlProps.translation).toMatchObject({
      wearablePreviewId: 'preview',
      wearablePreviewController: controller,
      vertical: true,
      verticalPosition: 'left'
    })
  })
})
