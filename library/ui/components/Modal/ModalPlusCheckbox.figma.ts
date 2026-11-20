// url=<DCL_FIGMA_FILE>?node-id=10204-4821
// source=components/Modal/Modal.tsx
// component=Modal
// contract=1
import figma from 'figma'

export default {
  // The link under the text is the caller's content; `checkbox` takes the label and the state.
  example: figma.tsx`<Modal title={title} description={description} checkbox={checkbox} actions={actions} onClose={onClose} closeButton={false}>{link}</Modal>`,
  imports: ["import { Modal } from '@dcl/one-ui'"],
  id: 'ModalPlusCheckbox'
}
