// url=<DCL_FIGMA_FILE>?node-id=9083-598
// source=components/Tag/Tag.tsx
// component=Tag
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

// The capture draws Live and Online beside the players' count and the friends' pictures: each of
// those is its own tag or the caller's content.
const variant = instance.getEnum('Property 1', {
  Live: 'live',
  Online: 'online',
  Streaming: 'streaming',
  Listening: 'listening',
  StreamingLarge: 'voiceStream',
  Featured: 'featured'
})
const count = instance.getEnum('Property 1', {
  Live: '',
  Online: ' count={count} countLabel={countLabel}',
  Streaming: ' count={count} countLabel={countLabel}',
  Listening: '',
  StreamingLarge: ' count={count} countLabel={countLabel}',
  Featured: ''
})

export default {
  example: figma.tsx`<Tag variant="${variant}"${count} />`,
  imports: ["import { Tag } from '@dcl/one-ui'"],
  id: 'Status',
  metadata: { nestable: true }
}
