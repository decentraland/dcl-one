// WearableCard — the shared WearableCard drawn with the HUD's art: the rarity's tile texture, the corner
// flap's shape, the equipped outline and the panel behind a hovered card.

import { WearableCard as SharedWearableCard, type WearableCardProps, type WearableCardRarity } from '@dcl/one-ui'
import flap from '../assets/backpack/corner-flap.webp'
import ring from '../assets/backpack/equipped-outline.webp'
import hoverPanel from '../assets/backpack/hover-bg.webp'
import { rarityTile } from './rarityArt'

export type Rarity = WearableCardRarity

export function WearableCard(props: Omit<WearableCardProps, 'art' | 'labels'>): React.JSX.Element {
  return <SharedWearableCard {...props} art={{ tile: rarityTile(props.rarity), flap, ring, hoverPanel }} />
}
