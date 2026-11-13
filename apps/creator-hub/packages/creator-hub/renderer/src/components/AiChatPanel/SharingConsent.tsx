import { useState } from 'react';
import CloseIcon from '@mui/icons-material/Close';
import { Button, IconButton, Menu, MenuItem, Tooltip } from 'decentraland-ui2';

import type { AiSharingView } from '/shared/types/ai';

import { t } from '/@/modules/store/translation/utils';

import { BillingBody, BillingCard, BillingTitle } from './component.styled';
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
    <BillingCard
      role="region"
      aria-label={t('editor.ai.sharing.card_title')}
    >
      <div className={styles.cardHeader}>
        <BillingTitle>{t('editor.ai.sharing.card_title')}</BillingTitle>
        <Tooltip title={t('editor.ai.sharing.not_now')}>
          <IconButton
            size="small"
            aria-label={t('editor.ai.sharing.not_now')}
            onClick={onShareNotNow}
          >
            <CloseIcon sx={{ fontSize: 16 }} />
          </IconButton>
        </Tooltip>
      </div>
      <BillingBody>
        {signedOut ? t('editor.ai.sharing.signin_body') : t('editor.ai.sharing.card_body')}
      </BillingBody>
      {!signedOut && (
        <>
          <button
            className={styles.detailsToggle}
            type="button"
            aria-expanded={showDetails}
            aria-controls="ai-sharing-details"
            onClick={() => setShowDetails(open => !open)}
          >
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
              <li>{t('editor.ai.sharing.details_control')}</li>
            </ul>
          )}
          <div className={styles.cardActions}>
            <Button
              variant="contained"
              size="small"
              onClick={() => onShareDecision?.(true)}
            >
              {t('editor.ai.sharing.accept')}
            </Button>
            <Button
              variant="contained"
              color="secondary"
              size="small"
              onClick={() => onShareDecision?.(false)}
            >
              {t('editor.ai.sharing.decline')}
            </Button>
          </div>
        </>
      )}
    </BillingCard>
  );
}

/** Always visible while sharing is on, so it is never on without the creator seeing it. */
export function SharingStatusChip({
  sharing,
  onShareDecision,
  onSetConversationShared,
}: SharingProps) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  if (sharing?.status !== 'on') return null;
  const excluded = sharing.conversationExcluded;
  const close = () => setAnchor(null);
  return (
    <>
      <button
        className={styles.chip}
        data-excluded={excluded}
        type="button"
        aria-label={`${excluded ? t('editor.ai.sharing.chip_excluded') : t('editor.ai.sharing.chip')}: ${t('editor.ai.sharing.chip_menu')}`}
        aria-haspopup="menu"
        aria-expanded={anchor !== null}
        onClick={event => setAnchor(event.currentTarget)}
      >
        {excluded ? t('editor.ai.sharing.chip_excluded') : t('editor.ai.sharing.chip')}
      </button>
      <Menu
        anchorEl={anchor}
        open={anchor !== null}
        onClose={close}
        disableScrollLock
      >
        <MenuItem
          onClick={() => {
            onSetConversationShared?.(excluded);
            close();
          }}
        >
          {excluded ? t('editor.ai.sharing.menu_include') : t('editor.ai.sharing.menu_exclude')}
        </MenuItem>
        <MenuItem
          onClick={() => {
            onShareDecision?.(false);
            close();
          }}
        >
          {t('editor.ai.sharing.menu_stop')}
        </MenuItem>
      </Menu>
    </>
  );
}
