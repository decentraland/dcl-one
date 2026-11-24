import {
  forwardRef,
  useCallback,
  useImperativeHandle,
  useState,
  type ComponentPropsWithoutRef,
  type ReactElement,
  type ReactNode,
} from 'react';
import { cx } from '/@/lib/cx';
import Button from '@ui/atoms/Button';
import Popover from '@ui/atoms/Popover';
import Tooltip from '@ui/atoms/Tooltip';

import { ChevronDownIcon } from '../Icons';

import styles from './SplitButton.module.css';

export type SplitButtonProps = Omit<ComponentPropsWithoutRef<'button'>, 'children'> & {
  /** The fill of both buttons: the brand colour or grey. */
  look?: 'primary' | 'secondary';
  startIcon?: ReactNode;
  children?: ReactNode;
  /** What the menu button opens under the pair. */
  extra: ReactNode;
  tooltip?: string;
  extraTooltip?: string;
  /** Gap between the pair and the popover, in pixels. */
  popoverOffset?: number;
  'data-testid'?: string;
};

/** Lets the caller close the popover, for an action in it that opens something else. */
export type SplitButtonHandle = {
  close: () => void;
};

// A disabled button shows no tooltip.
function withTooltip(
  title: string | undefined,
  disabled: boolean | undefined,
  button: ReactElement,
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

/** A main action and a menu button beside it that opens more options in a popover. */
export const SplitButton = forwardRef<SplitButtonHandle, SplitButtonProps>(function SplitButton(
  { look = 'primary', extra, tooltip, extraTooltip, popoverOffset = 0, className, ...props },
  ref,
) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const toggle = useCallback((event: React.MouseEvent<HTMLElement>) => {
    const target = event.currentTarget;
    setAnchor(previous => (previous ? null : target));
  }, []);
  const close = useCallback(() => setAnchor(null), []);

  useImperativeHandle(ref, () => ({ close }), [close]);

  return (
    <div
      className={cx('SplitButton', className)}
      role="group"
    >
      {withTooltip(
        tooltip,
        props.disabled,
        <Button
          {...props}
          className={`hub-button hub-button--${look}`}
        />,
      )}
      {withTooltip(
        extraTooltip,
        props.disabled,
        <Button
          className={`hub-button hub-button--${look} SplitButton-menu`}
          size="sm"
          disabled={props.disabled}
          aria-label={extraTooltip}
          aria-haspopup="true"
          aria-expanded={anchor !== null}
          onClick={toggle}
        >
          <ChevronDownIcon />
        </Button>,
      )}
      {anchor && (
        <Popover
          open
          anchor={anchor}
          onClose={close}
          placement="bottom-end"
          offset={popoverOffset}
          closeOnEscape={false}
          className={styles.popover}
        >
          {extra}
        </Popover>
      )}
    </div>
  );
});
