// url=<DCL_FIGMA_FILE>?node-id=7502-2312
// source=components/Toast/Toast.tsx
// component=Toast
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

// TextIconText draws a key cap inside the message: that is the caller's content.
const variant = instance.getEnum('Property 1', {
  TextOnly: 'text',
  TextIconText: 'text',
  IconText: 'confirmation',
  Error: 'error',
  ErrorClose: 'error',
  ErrorCTA: 'error'
})
const extra = instance.getEnum('Property 1', {
  TextOnly: '',
  TextIconText: '',
  IconText: '',
  Error: '',
  ErrorClose: ' closeButton',
  ErrorCTA: ' action={action}'
})

export default {
  example: figma.tsx`<Toast variant="${variant}" onClose={onClose}${extra}>{message}</Toast>`,
  imports: ["import { Toast } from '@dcl/one-ui'"],
  id: 'toast'
}
