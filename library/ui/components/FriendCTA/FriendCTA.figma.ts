// url=<DCL_FIGMA_FILE>?node-id=16534-723
// source=components/FriendCTA/FriendCTA.tsx
// component=FriendCTA
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

// Hover and pressed are looks the component draws for a state by itself. The design has no reject relationship.
const status = instance.getEnum('relationship', {
  add: 'add',
  'request-sent': 'sent',
  friend: 'friend',
  blocked: 'blocked'
})
const loading = instance.getEnum('state', {
  default: '',
  hover: '',
  pressed: '',
  loading: ' loading'
})
const viewport = instance.getEnum('viewport', { desktop: 'desktop', mobile: 'mobile' })

export default {
  example: figma.tsx`<FriendCTA status="${status}" viewport="${viewport}"${loading} />`,
  imports: ["import { FriendCTA } from '@dcl/one-ui'"],
  id: 'friend-cta'
}
