// url=<DCL_FIGMA_FILE>?node-id=8711-19210
// source=components/ProgressBar/ProgressBar.tsx
// component=ProgressBar
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

// The design draws a share of the track: the caller passes the progress, out of 100, and its text.
const value = instance.getEnum('status', { empty: '0', progress: 'value', completed: '100' })
const completed = instance.getEnum('status', {
  empty: '',
  progress: '',
  completed: ' completedLabel="Completed"'
})
const text = instance.getEnum('label', { none: '', end: ' valueLabel={valueText}' })

export default {
  // The bar's name is the caller's text: the design draws none.
  example: figma.tsx`<ProgressBar aria-label={label} value={${value}}${completed}${text} />`,
  imports: ["import { ProgressBar } from '@dcl/one-ui'"],
  id: 'progress-bar'
}
