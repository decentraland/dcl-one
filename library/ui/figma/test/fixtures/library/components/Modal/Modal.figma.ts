// url=<DCL_FIGMA_FILE>?node-id=15906-10331
// source=components/Modal/Modal.tsx
// component=Modal
// contract=1
// states=Property 1,Device,Scroll,Slot#2996:0,Slot 2#2996:16,Content#3888:0,Buttons#3889:16,Content2#15936:0,Content3#15936:5
import figma from 'figma'
const instance = figma.selectedInstance

const titleLayer = instance.findText('Buy Item')
const title = titleLayer.type === 'TEXT' ? titleLayer.textContent : ''

export default {
  example: figma.tsx`<Modal title="${title}" onClose={onClose} actions={actions}>{content}</Modal>`,
  imports: ["import { Modal } from '@dcl/one-ui'"],
  id: 'modal'
}
