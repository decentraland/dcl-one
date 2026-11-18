import { useState } from 'react';
import IconButton from '@ui/atoms/IconButton';
import Popover from '@ui/atoms/Popover';
import Tooltip from '@ui/atoms/Tooltip';
import {
  ChevronRightIcon,
  CloseIcon,
  ExpandMoreIcon,
  ThumbUpAltIcon,
} from '@ui/atoms/icons-material';

import type { AiSharingView } from '/shared/types/ai';

import { t } from '/@/modules/store/translation/utils';

import chatStyles from './component.module.css';
import styles from './SharingConsent.module.css';

export interface SharingProps {
  sharing?: AiSharingView;
  onShareDecision?: (granted: boolean) => void;
  onShareNotNow?: () => void;
  onSetConversationShared?: (shared: boolean) => void;
}

/**
 * The one-time ask. Both answers are equal-weight buttons; closing it is "not now", not "no",
 * so it comes back on the next launch.
 */
export function SharingConsentCard({ sharing, onShareDecision, onShareNotNow }: SharingProps) {
  const [showDetails, setShowDetails] = useState(false);
  if (sharing === undefined || (sharing.status !== 'ask' && sharing.status !== 'signin')) {
    return null;
  }
  const signedOut = sharing.status === 'signin';
  return (
    <div
      className={chatStyles.billingCard}
      role="region"
      aria-label={t('editor.ai.sharing.card_title')}
    >
      <div className={styles.cardHeader}>
        <span className={chatStyles.billingTitle}>{t('editor.ai.sharing.card_title')}</span>
        <Tooltip label={t('editor.ai.sharing.not_now')}>
          <IconButton
            size="small"
            className={styles.close}
            aria-label={t('editor.ai.sharing.not_now')}
            onClick={onShareNotNow}
          >
            <CloseIcon fontSize="small" />
          </IconButton>
        </Tooltip>
      </div>
      <span className={chatStyles.billingBody}>
        {signedOut ? t('editor.ai.sharing.signin_body') : t('editor.ai.sharing.card_body')}
      </span>
      {!signedOut && (
        <>
          <div className={styles.details}>
            <button
              className={styles.detailsToggle}
              type="button"
              aria-expanded={showDetails}
              aria-controls="ai-sharing-details"
              onClick={() => setShowDetails(open => !open)}
            >
              <ChevronRightIcon
                className={styles.detailsChevron}
                data-open={showDetails}
                aria-hidden
              />
              {t('editor.ai.sharing.details_toggle')}
            </button>
            {showDetails && (
              <ul
                className={styles.detailsList}
                id="ai-sharing-details"
              >
                <li>{t('editor.ai.sharing.details_shared')}</li>
                <li>{t('editor.ai.sharing.details_not_shared')}</li>
                <li>{t('editor.ai.sharing.details_analysis')}</li>
                <li>{t('editor.ai.sharing.details_publish')}</li>
                <li>{t('editor.ai.sharing.details_control')}</li>
              </ul>
            )}
          </div>
          <div className={styles.cardActions}>
            <button
              type="button"
              className={`${styles.action} ${styles.primary}`}
              onClick={() => onShareDecision?.(true)}
            >
              {t('editor.ai.sharing.accept')}
            </button>
            <button
              type="button"
              className={styles.action}
              onClick={() => onShareDecision?.(false)}
            >
              {t('editor.ai.sharing.decline')}
            </button>
          </div>
        </>
      )}
    </div>
  );
}

/**
 * Always visible while sharing is on, so it is never on without the creator seeing it. Opens two
 * switches: this chat (off also erases what it already shared) and every chat (off withdraws).
 */
export function SharingStatusChip({
  sharing,
  onShareDecision,
  onSetConversationShared,
}: SharingProps) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  if (sharing?.status !== 'on') return null;
  const excluded = sharing.conversationExcluded;
  const close = () => setAnchor(null);
  const label = excluded ? t('editor.ai.sharing.chip_excluded') : t('editor.ai.sharing.chip');
  return (
    <>
      <button
        className={styles.chip}
        type="button"
        aria-label={`${label}: ${t('editor.ai.sharing.chip_menu')}`}
        aria-haspopup="dialog"
        aria-expanded={anchor !== null}
        onClick={event => setAnchor(event.currentTarget)}
      >
        {!excluded && (
          <span
            className={styles.chipDot}
            aria-hidden
          />
        )}
        {label}
        <ExpandMoreIcon
          className={styles.chipChevron}
          aria-hidden
        />
      </button>
      <Popover
        anchor={anchor}
        open={anchor !== null}
        onClose={close}
        placement="bottom-end"
        offset={6}
        bare
        className={styles.menu}
      >
        <div
          role="group"
          aria-label={t('editor.ai.sharing.chip_menu')}
        >
          <div className={styles.menuRow}>
            <MenuSwitch
              label={t('editor.ai.sharing.menu_share_chat')}
              checked={!excluded}
              onChange={checked => onSetConversationShared?.(checked)}
            />
            {excluded && sharing.conversationHasShared === true && (
              <p className={styles.menuNote}>{t('editor.ai.sharing.menu_chat_erased')}</p>
            )}
          </div>
          <hr className={styles.menuDivider} />
          <div className={styles.menuRow}>
            <MenuSwitch
              label={t('editor.ai.sharing.menu_share_all')}
              checked
              onChange={checked => {
                if (checked) return;
                onShareDecision?.(false);
                close();
              }}
            />
          </div>
        </div>
      </Popover>
    </>
  );
}

/** One row of the sharing menu: its label and Figma's 34 x 20 switch, the whole row clickable. */
function MenuSwitch({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      className={styles.menuSwitch}
      onClick={() => onChange(!checked)}
    >
      <span>{label}</span>
      <span
        className={styles.switch}
        data-checked={checked}
        aria-hidden
      />
    </button>
  );
}

/**
 * Thumbs up or down on one shared reply; pressing the chosen one again clears it. Shown only
 * while the conversation is shared and the reply was shared with it.
 */
export function TurnRating({
  sharing,
  recordId,
  rating,
  onRate,
}: {
  sharing?: AiSharingView;
  recordId?: string;
  rating?: 'up' | 'down';
  onRate?: (rating: 'up' | 'down' | null) => void;
}) {
  if (
    sharing?.status !== 'on' ||
    sharing.conversationExcluded ||
    recordId === undefined ||
    onRate === undefined
  ) {
    return null;
  }
  const choices = [
    { value: 'up', label: t('editor.ai.sharing.rate_up') },
    { value: 'down', label: t('editor.ai.sharing.rate_down') },
  ] as const;
  return (
    <div className={styles.rating}>
      {choices.map(({ value, label }) => {
        const pressed = rating === value;
        return (
          <Tooltip
            key={value}
            label={label}
          >
            <IconButton
              size="small"
              className={styles.rateButton}
              aria-label={label}
              aria-pressed={pressed}
              onClick={() => onRate(pressed ? null : value)}
            >
              <ThumbUpAltIcon
                fontSize="inherit"
                data-direction={value}
              />
            </IconButton>
          </Tooltip>
        );
      })}
    </div>
  );
}
