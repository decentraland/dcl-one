import { createElement, type MouseEvent, type ReactNode } from 'react';
import Dialog, { type DialogCloseReason } from '@ui/atoms/Dialog';

import styles from './Modal.module.css';

export type ModalProps = {
  open: boolean;
  size?: 'tiny' | 'small' | 'medium' | 'large';
  /** Class of the root element, the one that covers the window. */
  className?: string;
  title?: string;
  subtitle?: string;
  /** Names a dialog that has no `title`. */
  'aria-label'?: string;
  'aria-labelledby'?: string;
  /** Goes on the dialog element. */
  'data-testid'?: string;
  header?: ReactNode;
  children: ReactNode;
  actions?: ReactNode;
  // Method signatures, so a handler that declares the event of a button still fits.
  /** The dialog gives the reason only: `event` is never set. */
  onClose?(event?: MouseEvent<HTMLButtonElement>, reason?: ModalCloseReason): void;
  onBack?(event?: MouseEvent<HTMLButtonElement>): void;
  /** Not applied: the look comes from the CSS Module. */
  sx?: unknown;
};

/** The reason names of the library the app had before; a press on the close button gives none. */
export type ModalCloseReason = 'backdropClick' | 'escapeKeyDown';

const closeReasons: Record<DialogCloseReason, ModalCloseReason | undefined> = {
  backdrop: 'backdropClick',
  escape: 'escapeKeyDown',
  close: undefined,
};

const closeIcon = createElement(
  'svg',
  {
    viewBox: '0 0 24 24',
    width: 24,
    height: 24,
    fill: 'currentColor',
    'aria-hidden': true,
    focusable: false,
  },
  createElement('path', {
    d: 'M12 2C6.47 2 2 6.47 2 12s4.47 10 10 10 10-4.47 10-10S17.53 2 12 2m4.3 14.3c-.39.39-1.02.39-1.41 0L12 13.41 9.11 16.3c-.39.39-1.02.39-1.41 0a.9959.9959 0 0 1 0-1.41L10.59 12 7.7 9.11a.9959.9959 0 0 1 0-1.41c.39-.39 1.02-.39 1.41 0L12 10.59l2.89-2.89c.39-.39 1.02-.39 1.41 0 .39.39.39 1.02 0 1.41L13.41 12l2.89 2.89c.38.38.38 1.02 0 1.41',
  }),
);

function classes(...names: (string | false | undefined)[]) {
  return names.filter(Boolean).join(' ');
}

export function Modal({
  open,
  size,
  className,
  title,
  subtitle,
  header,
  children,
  actions,
  onClose,
  onBack,
  'aria-label': ariaLabel,
  'aria-labelledby': ariaLabelledBy,
  'data-testid': testId,
}: ModalProps) {
  return createElement(
    Dialog,
    {
      open,
      size: size === 'large' ? 'medium' : size,
      title: title || undefined,
      titleAs: 'h5',
      subtitle: subtitle || undefined,
      header,
      actions: actions || undefined,
      onClose: onClose
        ? (reason: DialogCloseReason) => onClose(undefined, closeReasons[reason])
        : undefined,
      onBack: onBack ? () => onBack() : undefined,
      closeIcon,
      'aria-label': ariaLabel,
      'aria-labelledby': ariaLabelledBy,
      'data-testid': testId,
      className: classes(styles.paper, size === 'large' && styles.large),
      backdropClassName: classes(styles.root, className),
    },
    children,
  );
}

export { WarningModal } from './WarningModal';

export { TabsModal } from './TabsModal';
