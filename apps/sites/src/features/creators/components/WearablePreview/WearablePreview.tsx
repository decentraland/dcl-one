import { lazy, Suspense } from 'react'
import type { IPreviewController } from '@dcl/schemas'
import { WearablePreview as Ui2WearablePreview } from 'decentraland-ui2'
import type {
  EmoteControlsProps,
  WearablePreviewController,
  WearablePreviewProps,
  TranslationControlsProps
} from './types'

export function WearablePreview(props: WearablePreviewProps) {
  return <Ui2WearablePreview {...props} />
}

export function createWearablePreviewController(id: string): WearablePreviewController {
  return Ui2WearablePreview.createController(id)
}

// Every controller in the app comes from createWearablePreviewController, which returns ui2's whole one; its controls
// call more of it than the app's narrower type lists.
const full = (controller: WearablePreviewController | undefined) => controller as IPreviewController | undefined

// ui2's preview controls are MUI components: their styles read `theme.spacing()` and the Button
// reads `palette.mode`, and this app mounts no MUI theme. ui2's theme is a CSS-vars theme, so it
// has to go through CssVarsProvider, scoped to these subtrees so ui2's global resets stay out.
const loadTheme = () =>
  Promise.all([import('@mui/material/styles'), import('decentraland-ui2/dist/theme')]).then(
    ([{ Experimental_CssVarsProvider }, { light }]) => ({ CssVarsProvider: Experimental_CssVarsProvider, theme: light })
  )

const EmoteControlsLazy = lazy(async () => {
  const [{ EmoteControls }, { CssVarsProvider, theme }] = await Promise.all([
    import('decentraland-ui2/dist/components/WearablePreview/EmoteControls'),
    loadTheme()
  ])
  return {
    default: ({ previewId, controller, ...props }: EmoteControlsProps) => (
      <CssVarsProvider theme={theme}>
        <EmoteControls {...props} wearablePreviewId={previewId} wearablePreviewController={full(controller)} />
      </CssVarsProvider>
    )
  }
})

const TranslationControlsLazy = lazy(async () => {
  const [{ TranslationControls, VerticalPosition }, { CssVarsProvider, theme }] = await Promise.all([
    import('decentraland-ui2/dist/components/WearablePreview/TranslationControls'),
    loadTheme()
  ])
  return {
    default: ({ previewId, controller, verticalPosition, ...props }: TranslationControlsProps) => (
      <CssVarsProvider theme={theme}>
        <TranslationControls
          {...props}
          wearablePreviewId={previewId}
          wearablePreviewController={full(controller)}
          verticalPosition={verticalPosition === 'left' ? VerticalPosition.LEFT : VerticalPosition.RIGHT}
        />
      </CssVarsProvider>
    )
  }
})

export function EmoteControls(props: EmoteControlsProps) {
  return (
    <Suspense fallback={null}>
      <EmoteControlsLazy {...props} />
    </Suspense>
  )
}

export function TranslationControls(props: TranslationControlsProps) {
  return (
    <Suspense fallback={null}>
      <TranslationControlsLazy {...props} />
    </Suspense>
  )
}
