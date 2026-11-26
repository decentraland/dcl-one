// IconButton — the shared RailButton with the HUD's icons: a glyph by name, a piece of art, or the player's
// picture through the HUD's Avatar.

import { MaskIcon, RailButton, type RailButtonProps } from '@dcl/one-ui'
import { Avatar } from './Avatar'
import { Icon, type IconName } from './icons'

interface IconButtonProps extends Omit<RailButtonProps, 'icon' | 'portrait'> {
  icon: IconName
  /** Render this profile picture in place of the icon. */
  avatar?: { src?: string; name: string; color?: string }
  /** Art drawn at `size` in place of the icon: white-on-transparent takes the icon colour, `color`
   *  art is drawn as-is. */
  art?: { src: string; size: number; color?: boolean }
}

export function IconButton({ icon, avatar, art, ...rest }: IconButtonProps): React.JSX.Element {
  return (
    <RailButton
      {...rest}
      portrait={avatar != null}
      icon={
        avatar ? (
          <Avatar src={avatar.src} name={avatar.name} color={avatar.color} size={42} framed />
        ) : art?.color === true ? (
          <img src={art.src} alt="" width={art.size} height={art.size} draggable={false} />
        ) : art ? (
          <MaskIcon src={art.src} size={art.size} />
        ) : (
          <Icon name={icon} size={24} />
        )
      }
    />
  )
}
