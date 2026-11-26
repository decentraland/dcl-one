// ArmedButton — a Button that answers only once it has been up a moment (ARM_MS). For the confirm that spends
// or gives away what can't be taken back (SEND GIFT, SEND TIP, the Shop's BUY ITEM). It appears under a press the
// player just made: in a dialog opened from the app, or, for BUY ITEM, in place of the loading step's disabled
// button. A second click of that press, or one meant for what was there before, must not confirm. With an in-app
// wallet nothing else asks first: no wallet window opens. Until it arms it looks the same and is announced as
// disabled (aria-disabled), so the button doesn't flash a disabled state for half a second.

import { useEffect, useState } from 'react'
import { Button } from '@dcl/one-ui'

/** How long a confirm waits before it answers. */
export const ARM_MS = 500

/** Every prop of the HUD Button's `<button>` form (variant, size, radius and the element's own), with a plain onClick. */
type ArmedButtonProps = Omit<
  Extract<React.ComponentProps<typeof Button>, { appearance: 'hud'; control?: undefined; href?: undefined }>,
  'appearance' | 'onClick' | 'children' | 'label'
> & {
  onClick?: () => void
  children: Exclude<React.ReactNode, null | undefined | boolean>
}

export function ArmedButton({ onClick, ...props }: ArmedButtonProps): React.JSX.Element {
  const [armed, setArmed] = useState(false)
  useEffect(() => {
    const timer = setTimeout(() => setArmed(true), ARM_MS)
    return () => clearTimeout(timer)
  }, [])
  return <Button appearance="hud" {...props} onClick={armed ? onClick : undefined} aria-disabled={armed ? props['aria-disabled'] : true} />
}
