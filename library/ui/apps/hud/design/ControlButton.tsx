// ControlButton — the small icon control used across the HUD (close, back, menu, emoji toggle, count
// pills…), drawn by the shared Button.

import { Button } from '@dcl/one-ui'

type Variant = 'ghost' | 'solid' | 'dark' | 'faint'
type Shape = 'square' | 'circle' | 'pill'
type Size = 'sm' | 'md' | 'lg'

interface ControlButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** ghost = transparent→white-10 hover (default); solid = dark fill; dark / faint = black / clear with a faint wash on hover (chat). */
  variant?: Variant
  /** square (default), circle, or pill (for label + value like a count). */
  shape?: Shape
  /** md = 30px (default), sm = 26px, lg = 32px. */
  size?: Size
  /** Selected/pressed state. */
  active?: boolean
}

type SharedProps = Extract<React.ComponentProps<typeof Button>, { appearance: 'hud'; control: Shape; href?: undefined }>

export function ControlButton({ shape = 'square', ...rest }: ControlButtonProps): React.JSX.Element {
  return <Button {...({ ...rest, appearance: 'hud', control: shape } as SharedProps)} />
}
