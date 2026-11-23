// url=<DCL_FIGMA_FILE>?node-id=8711-19210
// source=components/ProgressBar/ProgressBar.tsx
// component=ProgressBar
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

const value = instance.getEnum('Status', { '0/3': '0', '1/3': '1', '2/3': '2', Completed: '3' })
const text = instance.getEnum('Status', {
  '0/3': ' valueLabel="0/3"',
  '1/3': ' valueLabel="1/3"',
  '2/3': ' valueLabel="2/3"',
  Completed: ' completedLabel="Completed"'
})

export default {
  // The bar's name is the caller's text: the design draws none.
  example: figma.tsx`<ProgressBar aria-label={label} value={${value}} max={3}${text} />`,
  imports: ["import { ProgressBar } from '@dcl/one-ui'"],
  id: 'progress-bar'
}
