// url=<DCL_FIGMA_FILE>?node-id=13113-504
// source=components/Modal/Modal.tsx
// component=Modal
// contract=1
import figma from 'figma'

export default {
  example: figma.tsx`<Modal title={title} description={description} image={image} actions={action} onClose={onClose} closeButton={false} />`,
  imports: ["import { Modal } from '@dcl/one-ui'"],
  id: 'ModalSingleCTA'
}
