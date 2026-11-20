import type { IconType } from 'react-icons';
import {
  GiBeard,
  GiBodySwapping,
  GiConverseShoe,
  GiCrestedHelmet,
  GiDominoMask,
  GiDropEarrings,
  GiEyeball,
  GiEyelashes,
  GiFeather,
  GiGloves,
  GiHairStrands,
  GiLips,
  GiShirt,
  GiSunglasses,
  GiTiara,
  GiTopHat,
  GiTrousers,
} from 'react-icons/gi';

import type { WearableCategory } from '../../../lib/logic/wearables';

const ICONS: Record<WearableCategory, IconType> = {
  helmet: GiCrestedHelmet,
  hat: GiTopHat,
  mask: GiDominoMask,
  eyewear: GiSunglasses,
  earring: GiDropEarrings,
  tiara: GiTiara,
  top_head: GiFeather,
  hair: GiHairStrands,
  eyes: GiEyeball,
  eyebrows: GiEyelashes,
  mouth: GiLips,
  facial_hair: GiBeard,
  upper_body: GiShirt,
  hands_wear: GiGloves,
  lower_body: GiTrousers,
  feet: GiConverseShoe,
  skin: GiBodySwapping,
};

export function CategoryIcon({
  category,
  className,
}: {
  category?: WearableCategory;
  className?: string;
}) {
  const Icon = category ? ICONS[category] : GiShirt;
  return (
    <Icon
      className={className}
      aria-hidden="true"
    />
  );
}
