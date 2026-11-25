// url=<DCL_FIGMA_FILE>?node-id=15916-718
// source=components/Modal/Modal.tsx
// component=Modal
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

// The capture holds no artwork: the caller supplies the picture at the size the design draws it.
const size = instance.getEnum('illustration', {
  review: 'width={262} height={148}',
  error: 'width={100} height={125}'
})

export default {
  example: figma.tsx`<Modal title={title} image={<img src={illustration} alt="" ${size} />} onClose={onClose} />`,
  imports: ["import { Modal } from '@dcl/one-ui'"],
  id: 'ModalIllustration',
  metadata: { nestable: true }
}
