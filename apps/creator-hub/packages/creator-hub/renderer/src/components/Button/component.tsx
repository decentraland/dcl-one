import { forwardRef, useCallback, useImperativeHandle, useState, type ForwardedRef } from 'react';
import { cx } from '/@/lib/cx';
import Ui3Button from '@ui/atoms/HubButton';
import Tooltip from '@ui/atoms/Tooltip';

import { ChevronDownIcon } from '../Icons';
import { Popper } from '../Popper';

import type { ButtonGroupHandle, ButtonProps, GroupProps } from './types';

import './styles.css';

const VARIANTS = { contained: 'primary', outlined: 'outline', text: 'text' } as const;
const SIZES = { small: 'sm', medium: 'md', large: 'lg' } as const;
const TONES = {
  primary: undefined,
  secondary: undefined,
  info: undefined,
  inherit: 'neutral',
  error: 'danger',
  success: 'success',
  warning: 'warning',
} as const;

// forwardRef so a caller can measure or focus the DOM node.
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { children, className = '', variant = 'text', color = 'primary', size = 'medium', ...props },
  ref,
) {
  return (
    <Ui3Button
      {...props}
      ref={ref as ForwardedRef<HTMLButtonElement | HTMLAnchorElement>}
      className={cx('Button', className)}
      variant={VARIANTS[variant]}
      size={SIZES[size]}
      tone={TONES[color]}
      data-variant={variant}
      data-color={color}
      data-size={size}
    >
      {children}
    </Ui3Button>
  );
});

// A disabled button has no tooltip, so it is rendered bare.
function withTooltip(
  title: string | undefined,
  disabled: boolean | undefined,
  button: React.ReactElement,
) {
  if (!title || disabled) return button;
  return (
    <Tooltip
      label={title}
      side="bottom"
      portal
      labelChild
    >
      {button}
    </Tooltip>
  );
}

// forwardRef so a caller can close the "extra" popover imperatively (e.g. a one-shot action
// inside it, like "Show QR Code for Mobile", that should dismiss the menu when it fires).
export const ButtonGroup = forwardRef<ButtonGroupHandle, GroupProps>(function ButtonGroup(
  { extra, tooltip, extraTooltip, popperOffset, ...props },
  ref,
) {
  const [open, setOpen] = useState(false);
  const [anchorEl, setAnchorEl] = useState<HTMLElement | null>(null);
  const handleToggle = useCallback((e: React.MouseEvent<HTMLElement>) => {
    setAnchorEl(e.currentTarget);
    setOpen(prev => !prev);
  }, []);

  const handleClose = useCallback(() => {
    setOpen(false);
    setAnchorEl(null);
  }, []);

  useImperativeHandle(ref, () => ({ close: handleClose }), [handleClose]);

  return (
    <div
      className="ButtonGroup"
      role="group"
      data-testid="button-group"
    >
      {withTooltip(
        tooltip,
        props.disabled,
        <Button
          variant="contained"
          {...props}
        />,
      )}
      {withTooltip(
        extraTooltip,
        props.disabled,
        <Button
          className="extra-button"
          variant="contained"
          color={props.color}
          size="small"
          disabled={props.disabled}
          data-testid="button-group-extra-button"
          onClick={handleToggle}
        >
          <ChevronDownIcon />
        </Button>,
      )}
      {open && (
        <Popper
          open={open}
          onClose={handleClose}
          anchorEl={anchorEl}
          placement="bottom-end"
          data-testid="button-group-popper"
          modifiers={
            popperOffset ? [{ name: 'offset', options: { offset: [0, popperOffset] } }] : undefined
          }
        >
          {extra}
        </Popper>
      )}
    </div>
  );
});
