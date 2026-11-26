// url=<DCL_FIGMA_FILE>?node-id=7502-2312
// source=components/Toast/Toast.tsx
// component=Toast
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

// Only icon-text has tones: its error tone is the error variant, and its neutral tone draws an info mark the
// component lacks, so it renders as text. The close and action types draw the error mark.
const kind = instance.getEnum('type', {
  text: ' variant="text"',
  'icon-text': '',
  'icon-action': ' variant="error" closeButton',
  'icon-two-actions': ' variant="error" action={action}'
})
const tone = instance.getEnum('tone', { neutral: '', error: ' variant="error"' })

export default {
  example: figma.tsx`<Toast${kind}${tone} onClose={onClose}>{message}</Toast>`,
  imports: ["import { Toast } from '@dcl/one-ui'"],
  id: 'toast'
}
