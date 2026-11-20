// url=<DCL_FIGMA_FILE>?node-id=8064-1628
// source=components/ProfilePic/ProfilePic.tsx
// component=ProfilePic
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

const size = instance.getEnum('Size', {
  '72': '72',
  '60': '60',
  '48': '48',
  '40': '40',
  '32': '32',
  '28': '28',
  '24': '24',
  '20': '20'
})

export default {
  example: figma.tsx`<ProfilePic name={name} src={src} size={${size}} />`,
  imports: ["import { ProfilePic } from '@dcl/one-ui'"],
  id: 'profile-pic'
}
