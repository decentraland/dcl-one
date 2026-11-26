// url=<DCL_FIGMA_FILE>?node-id=15916-889
// source=components/Modal/Modal.tsx
// component=Modal
// contract=1
// states=Device
import figma from 'figma'
const instance = figma.selectedInstance

// An info has one action and draws no title; an error draws the line above its two actions.
const kind = instance.getEnum('type', { info: ' titleHidden', error: ' divider' })
// The resolver takes an absent boolean as false, so this one adds nothing: an error whose title
// is off in the design needs `titleHidden` too.
const title = instance.getBoolean('title#15916:0', { true: '', false: '' })

export default {
  // The design has no close button: an action closes the dialog. The title always names it.
  example: figma.tsx`<Modal title={title} description={description} image={illustration} actions={actions} onClose={onClose} closeButton={false}${kind}${title} />`,
  imports: ["import { Modal } from '@dcl/one-ui'"],
  id: 'MessageModal'
}
