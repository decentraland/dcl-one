import type { ReactNode } from 'react';

export type Position =
  | 'top left'
  | 'top right'
  | 'top center'
  | 'bottom left'
  | 'bottom right'
  | 'bottom center'
  | 'left center'
  | 'right center';

export type Props = {
  text: ReactNode;
  link?: string;
  type?: 'info' | 'help' | 'warning' | 'error';
  trigger?: ReactNode;
  position?: Position;
  className?: string;
  disabled?: boolean;
  open?: boolean;
  hoverable?: boolean;
  hideOnScroll?: boolean;
  openOnTriggerMouseEnter?: boolean;
  closeOnTriggerClick?: boolean;
  onOpen?: () => void;
  onClose?: () => void;
};
