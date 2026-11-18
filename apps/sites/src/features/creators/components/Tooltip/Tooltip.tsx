import type { ReactElement, ReactNode } from 'react'
import * as S from './Tooltip.styles'

type Props = {
  /** Nothing to say (null) renders the children alone. */
  content: ReactNode
  /** The icon or glyph that opens the tooltip on hover, focus or tap; with `asChild`, the trigger element itself. */
  children: ReactNode
  /** Use the child as the trigger instead of wrapping it: it must be a single element that forwards its ref. */
  asChild?: boolean
  placement?: S.UiTooltipProps['placement']
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
  // remount it (and drop its focus); MUI renders no tooltip for an empty title.
  if (empty && !asChild) return <>{children}</>
  return (
    <S.Popper
      arrow
      describeChild={describeChild}
      title={empty ? '' : <S.Content data-testid={testId}>{content}</S.Content>}
      enterDelay={0}
      enterTouchDelay={0}
      leaveTouchDelay={4000}
      placement={placement}
    >
      {asChild ? (
        (children as ReactElement)
      ) : (
        <S.Trigger type="button" data-testid={`${testId}-trigger`}>
          {children}
        </S.Trigger>
      )}
    </S.Popper>
  )
}
