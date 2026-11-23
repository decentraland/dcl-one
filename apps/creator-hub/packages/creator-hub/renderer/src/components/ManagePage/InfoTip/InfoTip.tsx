import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type FocusEvent,
  type KeyboardEvent,
  type ReactNode,
} from 'react';

import { InfoIcon, WarningCircleIcon } from '../../Icons';

import styles from './InfoTip.module.css';

const CLOSE_GRACE_MS = 150;

type Variant = 'info' | 'warning';

const GLYPH: Record<Variant, ReactNode> = {
  info: <InfoIcon />,
  warning: <WarningCircleIcon size={18} />,
};

type Placed = { shift: number; lift: number; below: boolean };

const OVER_AND_CENTRED: Placed = { shift: 0, lift: 0, below: false };

type Props = {
  label: string;
  variant?: Variant;
  size?: 'small' | 'medium';
  children: ReactNode;
};

/** A toggletip behind an ⓘ or ⚠ glyph: opens on hover, focus or click; Escape, click-away or leaving it closes it. */
export function InfoTip({ label, variant = 'info', size = 'medium', children }: Props) {
  const [open, setOpen] = useState(false);
  const [placed, setPlaced] = useState(OVER_AND_CENTRED);
  const wrapper = useRef<HTMLDivElement>(null);
  const anchor = useRef<HTMLButtonElement>(null);
  const tip = useRef<HTMLDivElement>(null);
  const closeTimer = useRef<ReturnType<typeof setTimeout>>();
  const dismissed = useRef(false);
  const bubbleId = useId();

  const show = () => {
    clearTimeout(closeTimer.current);
    dismissed.current = false;
    setOpen(true);
  };
  const hide = () => {
    clearTimeout(closeTimer.current);
    setOpen(false);
  };

  useEffect(() => {
    if (!open) return;
    const handleClick = (event: MouseEvent) => {
      if (!wrapper.current?.contains(event.target as Node)) hide();
    };
    document.addEventListener('click', handleClick);
    return () => document.removeEventListener('click', handleClick);
  }, [open]);

  // The tip is inside the wrapper, so the keyboard reaches its links.
  useLayoutEffect(() => {
    if (!open || !tip.current) {
      setPlaced(OVER_AND_CENTRED);
      return;
    }
    const rect = tip.current.getBoundingClientRect();
    const glyph = (anchor.current ?? tip.current).getBoundingClientRect();
    const parent = wrapper.current?.offsetParent;
    const frame = parent?.getBoundingClientRect();
    const originX = frame?.left ?? 0;
    const originY = frame?.top ?? 0;
    // Whole pixels from the positioned ancestor: the left edge from its left, the bottom edge
    // from its bottom. The bubble and its text keep their place between two openings.
    const centred =
      Math.round(glyph.left - originX + glyph.width / 2 - tip.current.offsetWidth / 2) + originX;
    const base =
      (frame?.bottom ?? 0) + Math.round(glyph.top - originY - (parent?.clientHeight ?? 0));
    const overflowRight = centred + rect.width - document.documentElement.clientWidth;
    const left = centred < 0 ? 0 : overflowRight > 0 ? centred - overflowRight : centred;
    const below = base - rect.height < 0;
    const shift = left - rect.left;
    const lift = below ? 0 : base - rect.bottom;
    if (shift || lift || below) setPlaced({ shift, lift, below });
  }, [open]);

  const handleFocus = () => {
    if (!dismissed.current) show();
  };
  const handleBlur = (event: FocusEvent) => {
    if (!event.currentTarget.contains(event.relatedTarget)) {
      dismissed.current = false;
      hide();
    }
  };
  const handleKeyDown = (event: KeyboardEvent) => {
    if (event.key !== 'Escape' || !open) return;
    event.stopPropagation();
    dismissed.current = true;
    hide();
    anchor.current?.focus();
  };

  return (
    <div
      ref={wrapper}
      className={styles.wrapper}
      onMouseEnter={show}
      onMouseLeave={() => {
        closeTimer.current = setTimeout(hide, CLOSE_GRACE_MS);
      }}
      onFocus={handleFocus}
      onBlur={handleBlur}
      onKeyDown={handleKeyDown}
    >
      <button
        ref={anchor}
        type="button"
        className={styles.button}
        data-look={variant}
        data-size={size}
        aria-label={label}
        aria-expanded={open}
        aria-controls={bubbleId}
        onClick={show}
      >
        {GLYPH[variant]}
      </button>
      {open && (
        <div
          ref={tip}
          className={styles.tip}
          role="presentation"
          data-placement={placed.below ? 'bottom' : 'top'}
          style={
            {
              '--tip-shift': `${placed.shift}px`,
              '--tip-lift': `${placed.lift}px`,
            } as CSSProperties
          }
        >
          <div
            id={bubbleId}
            className={styles.bubble}
            onClick={event => {
              if ((event.target as HTMLElement).closest('a, button')) hide();
            }}
          >
            {children}
            <span className={styles.peak} />
          </div>
        </div>
      )}
    </div>
  );
}
