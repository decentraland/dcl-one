// url=<DCL_FIGMA_FILE>?node-id=12877-3067
// source=components/ProfilePic/ConnectionStatus.tsx
// component=ConnectionStatus
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

const status = instance.getEnum('Property 1', { Online: 'online', Away: 'away' })

export default {
  example: figma.tsx`<ConnectionStatus status="${status}" />`,
  imports: ["import { ConnectionStatus } from '@dcl/one-ui'"],
  id: 'connection-status',
  metadata: { nestable: true }
}
