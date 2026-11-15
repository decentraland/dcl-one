import type { ButtonHTMLAttributes, ReactNode } from 'react';

export type ButtonProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'color'> & {
  variant?: 'contained' | 'outlined' | 'text';
  color?: 'primary' | 'secondary' | 'info' | 'inherit' | 'error' | 'success' | 'warning';
  size?: 'small' | 'medium' | 'large';
  startIcon?: ReactNode;
  endIcon?: ReactNode;
  fullWidth?: boolean;
  'data-testid'?: string;
};

export type GroupProps = ButtonProps & {
  extra: React.ReactNode;
  tooltip?: string;
  extraTooltip?: string;
  // Vertical gap (px) between the button and the "extra" popover — unset keeps it flush,
  // matching every other ButtonGroup (e.g. Publish options).
  popperOffset?: number;
};

// Lets a caller close the "extra" popover imperatively — e.g. when a one-shot action inside
// it (like "Show QR Code for Mobile") should dismiss the menu instead of leaving it open
// behind whatever the action opens.
export type ButtonGroupHandle = {
  close: () => void;
};
