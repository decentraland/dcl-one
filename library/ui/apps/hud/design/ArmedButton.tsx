// ArmedButton — a Button that answers only once it has been up a moment (ARM_MS). For the confirm that spends
// or gives away what can't be taken back (SEND GIFT, SEND TIP): it appears in a dialog the player opened with a
// press in the app, and a second click of that press, or one meant for what was there before, must not confirm.
// With an in-app wallet nothing else asks first: no wallet window opens. Until it arms it looks the same and is
// announced as disabled (aria-disabled), so the button doesn't flash a disabled state for half a second.

import { useEffect, useState } from 'react'
import { Button } from './Button'

/** How long a confirm waits before it answers. The Shop's BUY ITEM waits as long, in its own copy
 *  (ShopPurchaseDialog's ArmedButton). */
export const ARM_MS = 500

/** Every prop of Button's `<button>` form (variant, size, radius and the element's own), with a plain onClick. */
type ArmedButtonProps = Omit<Extract<React.ComponentProps<typeof Button>, { href?: undefined }>, 'onClick'> & {
  onClick?: () => void
}

export function ArmedButton({ onClick, ...props }: ArmedButtonProps): React.JSX.Element {
  const [armed, setArmed] = useState(false)
  useEffect(() => {
    const timer = setTimeout(() => setArmed(true), ARM_MS)
    return () => clearTimeout(timer)
  }, [])
  return <Button {...props} onClick={armed ? onClick : undefined} aria-disabled={armed ? props['aria-disabled'] : true} />
}
