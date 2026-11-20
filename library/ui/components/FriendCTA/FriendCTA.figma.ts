// url=<DCL_FIGMA_FILE>?node-id=8071-1665
// source=components/FriendCTA/FriendCTA.tsx
// component=FriendCTA
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

// The hover, pressed and action variants are the looks the component draws for a state by itself.
const status = instance.getEnum('Property 1', {
  Add: 'add',
  AddFriendHover: 'add',
  AddFriendLoading: 'add',
  Sent: 'sent',
  CancelRequest: 'sent',
  CancelRequestLoading: 'sent',
  Friend: 'friend',
  Unfriend: 'friend',
  UnfriendLoading: 'friend',
  Reject: 'reject',
  RejectHover: 'reject',
  RejectHoverPress: 'reject',
  Blocked: 'blocked',
  BlockedHover: 'blocked',
  BlockedPres: 'blocked'
})
const loading = instance.getEnum('Property 1', {
  Add: '',
  AddFriendHover: '',
  AddFriendLoading: ' loading',
  Sent: '',
  CancelRequest: '',
  CancelRequestLoading: ' loading',
  Friend: '',
  Unfriend: '',
  UnfriendLoading: ' loading',
  Reject: '',
  RejectHover: '',
  RejectHoverPress: '',
  Blocked: '',
  BlockedHover: '',
  BlockedPres: ''
})
const viewport = instance.getEnum('viewport', { desktop: 'desktop', mobile: 'mobile' })

export default {
  example: figma.tsx`<FriendCTA status="${status}" viewport="${viewport}"${loading} />`,
  imports: ["import { FriendCTA } from '@dcl/one-ui'"],
  id: 'friend-cta'
}
