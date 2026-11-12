import { useCallback, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import cx from 'classnames';
import CloseIcon from '@mui/icons-material/Close';
import { Button } from 'decentraland-ui2';

import { analytics } from '#preload';
import artwork from '/assets/images/earn/home-banner.webp';
import { useFeatureFlags } from '/@/hooks/useFeatureFlags';
import { FeatureFlag } from '/@/modules/store/featureFlags';
import { t } from '/@/modules/store/translation/utils';

import styles from './CreatorFundBanner.module.css';

/** Once closed, the banner stays closed on this machine. */
export const BANNER_DISMISSED_KEY = 'creator-hub:creator-fund-banner-dismissed';

function readDismissed() {
  try {
    return localStorage.getItem(BANNER_DISMISSED_KEY) === 'true';
  } catch {
    return false;
  }
}

function saveDismissed() {
  try {
    localStorage.setItem(BANNER_DISMISSED_KEY, 'true');
  } catch {
    // Storage can be unavailable; the banner still closes for this session.
  }
}

/** The Home promo for the Creator Fund, shown under the `creatorhub-creator-fund` flag. */
export function CreatorFundBanner() {
  const navigate = useNavigate();
  const { isEnabled } = useFeatureFlags();
  const [dismissed, setDismissed] = useState(readDismissed);

  const handleLearnMore = useCallback(() => {
    void analytics.track('Creator Fund Banner Clicked', undefined);
    navigate('/earn');
  }, [navigate]);

  const handleClose = useCallback(() => {
    void analytics.track('Creator Fund Banner Dismissed', undefined);
    saveDismissed();
    setDismissed(true);
  }, []);

  if (dismissed || !isEnabled(FeatureFlag.CREATOR_FUND)) return null;

  return (
    <section
      className={styles.banner}
      aria-labelledby="creator-fund-banner-title"
    >
      <div
        className={cx(styles.glow, styles.bottomGlow)}
        aria-hidden
      />
      <div className={styles.information}>
        <h2
          className={styles.title}
          id="creator-fund-banner-title"
        >
          {t('home.creator_fund_banner.title')}
        </h2>
        <p className={styles.description}>{t('home.creator_fund_banner.description')}</p>
        <Button
          className={styles.cta}
          variant="contained"
          color="primary"
          disableRipple
          onClick={handleLearnMore}
        >
          {t('home.creator_fund_banner.cta')}
        </Button>
      </div>
      <div
        className={styles.artworkSlot}
        aria-hidden
      >
        <div className={cx(styles.glow, styles.topGlow)} />
        <img
          className={styles.artwork}
          src={artwork}
          alt=""
        />
      </div>
      <button
        className={styles.closeButton}
        type="button"
        aria-label={t('home.creator_fund_banner.close')}
        onClick={handleClose}
      >
        <CloseIcon />
      </button>
    </section>
  );
}
