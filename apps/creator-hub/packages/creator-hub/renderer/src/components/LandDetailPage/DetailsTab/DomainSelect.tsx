import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import type { MouseEvent, ReactNode } from 'react';
import { cx } from '/@/lib/cx';
import { Select } from '@ui/atoms/Select';

import { ChevronDownIcon } from '../../Icons';
import viewBar from '../../ManagePage/ViewBar/ViewBar.module.css';

import styles from './DetailsTab.module.css';

type Props = {
  value: string;
  onChange: (value: string) => void;
  'aria-label': string;
  /** What the field shows while `value` is empty. */
  placeholder?: ReactNode;
  renderValue?: (value: string) => ReactNode;
  defaultOpen?: boolean;
  className?: string;
  /** `SelectOption` elements. */
  children?: ReactNode;
};

/** The 46px outlined select of the detail tabs; its list has the look of the ViewBar lists. */
export function DomainSelect({ defaultOpen = false, className, children, ...props }: Props) {
  const [open, setOpen] = useState(defaultOpen);
  const pressed = useRef(false);
  const target = useRef<HTMLSpanElement>(null);
  // The field owns the keys of the list. A list that is open from the start has no press that gives it the focus.
  useEffect(() => {
    if (!defaultOpen) return;
    target.current?.querySelector<HTMLElement>('[role="combobox"]')?.focus({ preventScroll: true });
  }, []);
  const onTrigger = (event: MouseEvent<HTMLSpanElement>) =>
    event.target instanceof Element && event.target.closest('[role="combobox"]') !== null;

  return (
    // The select opens on the press, not on the release. The click of the same press must not close it.
    <span
      ref={target}
      className={styles.pressTarget}
      onMouseDownCapture={event => {
        if (open || event.button !== 0 || !onTrigger(event)) return;
        pressed.current = true;
        setOpen(true);
      }}
      onClickCapture={event => {
        if (!pressed.current || !onTrigger(event)) return;
        pressed.current = false;
        event.stopPropagation();
      }}
    >
      {open && createPortal(<div className={styles.menuBackdrop} />, document.body)}
      <Select
        {...props}
        className={cx(styles.domainSelect, className)}
        menuClassName={cx(viewBar.menu, styles.domainMenu)}
        placement="bottom"
        icon={<ChevronDownIcon />}
        open={open}
        onOpenChange={next => {
          if (!next) pressed.current = false;
          setOpen(next);
        }}
      >
        {children}
      </Select>
    </span>
  );
}
