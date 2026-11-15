import type { HTMLAttributes } from 'react';
import type { OverlayPlacement } from '@ui/atoms/useOverlayPosition';

export type Props = Omit<HTMLAttributes<HTMLDivElement>, 'children'> & {
  open: boolean;
  onClose: () => void;
  anchorEl: Element | null | undefined;
  placement?: OverlayPlacement;
  /** Only the `offset` modifier is read: its second number is the gap to the anchor, in pixels. */
  modifiers?: ReadonlyArray<{ name: string; options?: { offset?: readonly [number, number] } }>;
  'data-testid'?: string;
};
