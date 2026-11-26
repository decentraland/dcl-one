import { useRef, useState } from 'react';

import { Tooltip } from '@dcl/one-ui';
import { useDismiss } from '@ui/atoms/useDismiss';

import { misc } from '#preload';
import type { TranslationPath } from '/@/modules/store/translation/types';
import { t } from '/@/modules/store/translation/utils';

import { InfoIcon } from '../icons';

import { EARN_TOOLTIP } from './parts';
import parts from './parts.module.css';
import styles from './PayoutsTab.module.css';

/**
 * An info button whose interactive tooltip explains something and ends in "Learn More". The
 * tooltip is drawn at the end of the page, so Tab is moved by hand: from the button into "Learn
 * More", and from "Learn More" on from the button. It stays open while focus is in either.
 * While open it is the top overlay layer, so inside a dialog Escape closes the tooltip, not the
 * dialog.
 * @param props.label - the button's accessible name.
 * @param props.text - the tooltip text before "Learn More".
 * @param props.href - the page "Learn More" opens in the browser.
 */
export function InfoTip({
  label,
  text,
  href,
}: {
  label: TranslationPath;
  text: TranslationPath;
  href: string;
}) {
  const [open, setOpen] = useState(false);
  const [button, setButton] = useState<HTMLButtonElement | null>(null);
  const linkRef = useRef<HTMLAnchorElement | null>(null);
  const tipRef = useRef<HTMLElement | null>(null);
  const returning = useRef(false);
  const onOpenChange = (next: boolean) => {
    if (returning.current) return;
    if (!next && linkRef.current?.contains(document.activeElement)) {
      returning.current = true;
      button?.focus();
      returning.current = false;
    }
    setOpen(next);
  };
  useDismiss(open, {
    surface: tipRef,
    anchor: button,
    onEscape: () => {
      if (linkRef.current?.contains(document.activeElement)) button?.focus();
      setOpen(false);
    },
  });
  return (
    <span className={styles.infoGroup}>
      <Tooltip
        {...EARN_TOOLTIP}
        open={open}
        onOpenChange={onOpenChange}
        interactive
        anchor={button}
        content={
          <>
            {t(text)}{' '}
            <a
              ref={node => {
                linkRef.current = node;
                tipRef.current = node?.closest<HTMLElement>('[role="tooltip"]') ?? null;
              }}
              className={styles.learnMore}
              href={href}
              onClick={event => {
                event.preventDefault();
                void misc.openExternal(href);
              }}
              onKeyDown={event => {
                if (event.key !== 'Tab') return;
                if (event.shiftKey) event.preventDefault();
                button?.focus();
              }}
            >
              {t('earn.joined.payouts.wallet.learn_more')}
              <span aria-hidden> →</span>
            </a>
          </>
        }
      >
        <button
          ref={setButton}
          className={parts.infoButton}
          type="button"
          aria-label={t(label)}
          onKeyDown={event => {
            if (event.key !== 'Tab' || event.shiftKey || !open || !linkRef.current) return;
            event.preventDefault();
            linkRef.current.focus();
          }}
        >
          <InfoIcon />
        </button>
      </Tooltip>
    </span>
  );
}
