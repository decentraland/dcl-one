// url=<DCL_FIGMA_FILE>?node-id=9373-1555
// source=components/CommunityCard/CommunityCard.tsx
// component=CommunityCard
// contract=1
// states=Property 1
import figma from 'figma'
const instance = figma.selectedInstance

const nameLayer = instance.findText('Community Name')
const name = nameLayer.type === 'TEXT' ? nameLayer.textContent : ''

export default {
  example: figma.tsx`<CommunityCard name="${name}" href={href} image={image} ownerName={ownerName} privacy={privacy} membersLabel={membersLabel} description={description} />`,
  imports: ["import { CommunityCard } from '@dcl/one-ui'"],
  id: 'community-card'
}
