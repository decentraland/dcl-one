import styled from '@emotion/styled'
import { Tooltip as UiTooltip } from 'decentraland-ui2'
import { createElement, type ComponentProps } from 'react'

export type UiTooltipProps = ComponentProps<typeof UiTooltip>

// MUI applies `className` to the trigger, so route it to the portalled popper to style the bubble.
export const Popper = styled(({ className, ...props }: UiTooltipProps) =>
  createElement(UiTooltip, { ...props, classes: { popper: className } })
)`
  z-index: var(--z-tooltip);

  .MuiTooltip-tooltip {
    max-width: min(320px, calc(100vw - 32px));
    padding: 10px 12px;
    border-radius: var(--radius-btn-sm);
    background: var(--color-text2);
    color: var(--color-white);
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 400;
    line-height: 1.4;
    white-space: pre-line;
    box-shadow: 0 8px 24px var(--color-overlay-strong);
  }

  .MuiTooltip-arrow {
    color: var(--color-text2);
  }
`

export const Trigger = styled.button`
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  background: none;
  color: inherit;
  border-radius: var(--radius-chip);

  &:focus-visible {
    outline: 2px solid var(--color-glass-hover);
    outline-offset: 2px;
  }
`

export const Content = styled.span`
  color: var(--color-soft-white);
  font-size: 16px;
`
