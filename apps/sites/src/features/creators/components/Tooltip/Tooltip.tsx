import type { ReactNode } from 'react'
import UiTooltip from '@ui/atoms/Tooltip'
import styles from './Tooltip.module.css'

const MAX_WIDTH = 320

type Props = {
  /** Nothing to say (null) renders the children alone. */
  content: ReactNode
  /** The icon or glyph that opens the tooltip on hover, focus or tap; with `asChild`, the trigger element itself. */
  children: ReactNode
  /** Use the child as the trigger instead of wrapping it in a button. */
  asChild?: boolean
  placement?: 'top' | 'bottom' | 'left' | 'right'
  /** The tooltip describes the trigger instead of naming it, so its visible label stays its accessible name. */
  describeChild?: boolean
  testId?: string
}

export function Tooltip({
  content,
  children,
  asChild = false,
  describeChild = false,
  testId = 'tooltip',
  placement = 'top'
}: Props) {
  const empty = content === null || content === undefined || content === false
  // A trigger of its own keeps its wrapper while there is nothing to say, so content arriving later does not
  // remount it (and drop its focus).
  if (empty && !asChild) return <>{children}</>
  return (
    <UiTooltip
      content={
        <span className={styles.content} data-testid={testId}>
          {content}
        </span>
      }
      disabled={empty}
      side={placement}
      portal
      keepInViewport
      arrow
      maxWidth={MAX_WIDTH}
      tipClassName={styles.tip}
      labelChild={!describeChild}
    >
      {asChild ? (
        children
      ) : (
        <button type="button" className={styles.trigger} data-testid={`${testId}-trigger`}>
          {children}
        </button>
      )}
    </UiTooltip>
  )
}
