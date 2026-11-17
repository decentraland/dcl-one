// url=<DCL_FIGMA_FILE>?node-id=30-1
// source=components/Modal/Modal.tsx
// component=Modal
// contract=1
// states=Size,Scroll
import figma from 'figma'
const instance = figma.selectedInstance

const titleLayer = instance.findText('Dialog Title')
const title = titleLayer.type === 'TEXT' ? titleLayer.textContent : ''

export default {
  example: figma.tsx`<Modal title="${title}" onClose={onClose} actions={actions}>{content}</Modal>`,
  imports: ["import { Modal } from '@dcl/one-ui'"],
  id: 'modal'
}
