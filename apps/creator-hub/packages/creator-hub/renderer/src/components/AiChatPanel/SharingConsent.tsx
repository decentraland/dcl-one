import { useId, useState } from 'react';
import Button from '@ui/atoms/Button';
import IconButton from '@ui/atoms/IconButton';
import Popover from '@ui/atoms/Popover';
import Toggle from '@ui/atoms/Toggle';
import { Tooltip } from '@dcl/one-ui';
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
  // The card can show in the docked panel and the detached window at once.
  const detailsId = useId();
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
              aria-controls={detailsId}
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
                id={detailsId}
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
            <Button
              size="sm"
              className={styles.accept}
              onClick={() => onShareDecision?.(true)}
            >
              {t('editor.ai.sharing.accept')}
            </Button>
            <Button
              variant="secondary"
              tone="neutral"
              size="sm"
              className={styles.decline}
              onClick={() => onShareDecision?.(false)}
            >
              {t('editor.ai.sharing.decline')}
            </Button>
          </div>
        </>
      )}
    </div>
  );
}

/**
 * Always visible while sharing is on, so it is never on without the creator seeing it. Opens two
 * switches: this chat (off also erases what it already shared, so it asks first when there is
 * something to erase) and every chat (off withdraws).
 */
export function SharingStatusChip({
  sharing,
  onShareDecision,
  onSetConversationShared,
}: SharingProps) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const [confirmingErase, setConfirmingErase] = useState(false);
  const confirmId = useId();
  if (sharing?.status !== 'on') return null;
  const excluded = sharing.conversationExcluded;
  const close = () => {
    setAnchor(null);
    setConfirmingErase(false);
  };
  const setThisChatShared = (shared: boolean) => {
    // The switch reads off while the confirmation is open, but the chat is still shared: turning
    // it back on is a cancel, not a write.
    if (confirmingErase) {
      if (shared) setConfirmingErase(false);
      return;
    }
    if (!shared && sharing.conversationHasShared === true) setConfirmingErase(true);
    else onSetConversationShared?.(shared);
  };
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
      >
        <div
          role="group"
          className={styles.menu}
          aria-label={t('editor.ai.sharing.chip_menu')}
        >
          <div className={styles.menuRow}>
            <MenuSwitch
              label={t('editor.ai.sharing.menu_share_chat')}
              checked={!excluded && !confirmingErase}
              onChange={setThisChatShared}
            />
            {confirmingErase && (
              <div
                role="alertdialog"
                aria-labelledby={`${confirmId}-text`}
                className={styles.menuConfirm}
              >
                <p
                  id={`${confirmId}-text`}
                  className={styles.menuNote}
                >
                  {t('editor.ai.sharing.menu_chat_confirm')}
                </p>
                <div className={styles.menuConfirmActions}>
                  <Button
                    variant="secondary"
                    tone="neutral"
                    size="sm"
                    onClick={() => setConfirmingErase(false)}
                  >
                    {t('editor.ai.sharing.menu_chat_confirm_no')}
                  </Button>
                  <Button
                    size="sm"
                    tone="danger"
                    onClick={() => {
                      setConfirmingErase(false);
                      onSetConversationShared?.(false);
                    }}
                  >
                    {t('editor.ai.sharing.menu_chat_confirm_yes')}
                  </Button>
                </div>
              </div>
            )}
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

/** One row of the sharing menu: its label and the small switch; a click on the label flips it. */
function MenuSwitch({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  const labelId = useId();
  return (
    <label className={styles.menuSwitch}>
      <span id={labelId}>{label}</span>
      <Toggle
        size="small"
        checked={checked}
        ariaLabelledBy={labelId}
        onChange={onChange}
      />
    </label>
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
                className={value === 'down' ? styles.thumbDown : undefined}
              />
            </IconButton>
          </Tooltip>
        );
      })}
    </div>
  );
}
