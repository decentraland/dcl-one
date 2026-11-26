// HeaderButton — the dark round (or pill) buttons on a full-screen header: a 40px circle for an
// icon, or a 46px pill for a label such as the profile chip. Drawn by the shared Button.

import { Button } from '@dcl/one-ui'

type Shape = 'circle' | 'pill'
type SharedProps = Extract<React.ComponentProps<typeof Button>, { appearance: 'hud'; control: 'header' | 'headerPill'; href?: undefined }>

export function HeaderButton({
  shape = 'circle',
  ...rest
}: React.ButtonHTMLAttributes<HTMLButtonElement> & { shape?: Shape }): React.JSX.Element {
  return <Button {...({ ...rest, appearance: 'hud', control: shape === 'pill' ? 'headerPill' : 'header' } as SharedProps)} />
}
