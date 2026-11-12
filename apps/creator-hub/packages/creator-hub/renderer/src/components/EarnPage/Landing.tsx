import { useState } from 'react';
import { Button } from 'decentraland-ui2';

import { analytics, misc } from '#preload';

import heroImage from '/assets/images/earn/hero.webp';
import moneyMouth from '/assets/images/earn/money-mouth.webp';

import type { TranslationPath } from '/@/modules/store/translation/types';
import { getCurrentLocale, t } from '/@/modules/store/translation/utils';
import { CONTACT_SUPPORT_URL, CREATOR_FUND_URL } from '/@/modules/utils';

import type { CommonProps } from '../Icons/types';

import {
  AlertIcon,
  AutomaticEnrolmentIcon,
  CalendarIcon,
  PlayerActivityIcon,
  WalletIcon,
  WarningIcon,
} from './icons';
import { SnackbarAction, StatusSnackbar } from './StatusSnackbar';
import { TermsModal } from './TermsModal';
import styles from './Landing.module.css';

const FEATURES: {
  key: string;
  Icon: (props: CommonProps) => React.ReactElement;
  title: TranslationPath;
  body: TranslationPath;
}[] = [
  {
    key: 'enrolment',
    Icon: AutomaticEnrolmentIcon,
    title: 'earn.landing.features.enrolment.title',
    body: 'earn.landing.features.enrolment.body',
  },
  {
    key: 'cycles',
    Icon: CalendarIcon,
    title: 'earn.landing.features.cycles.title',
    body: 'earn.landing.features.cycles.body',
  },
  {
    key: 'activity',
    Icon: PlayerActivityIcon,
    title: 'earn.landing.features.activity.title',
    body: 'earn.landing.features.activity.body',
  },
  {
    key: 'payout',
    Icon: WalletIcon,
    title: 'earn.landing.features.payout.title',
    body: 'earn.landing.features.payout.body',
  },
];

export type LandingAudience = 'signed_in' | 'signed_out';

const COPY: Record<LandingAudience, { tagline: TranslationPath; cta: TranslationPath }> = {
  signed_in: { tagline: 'earn.landing.tagline_signed_in', cta: 'earn.landing.join' },
  signed_out: { tagline: 'earn.landing.tagline_signed_out', cta: 'earn.landing.sign_in_to_join' },
};

/**
 * The EARN landing for a creator who hasn't joined.
 * @param props.children - the CTA area: {@link LandingCtas} or {@link LandingLoadError}.
 */
export function Landing({
  audience,
  children,
}: {
  audience: LandingAudience;
  children: React.ReactNode;
}) {
  const copy = COPY[audience];

  return (
    <section className={styles.hero}>
      <img
        className={styles.heroImage}
        src={heroImage}
        alt=""
        aria-hidden
        width={1425}
        height={965}
        {...{ fetchpriority: 'high' }}
      />
      <div className={styles.information}>
        <div className={styles.intro}>
          <div className={styles.titleBlock}>
            <h1 className={styles.title}>
              {t('earn.landing.title_prefix')}
              <span className={styles.titleDot}> · </span>
              {t('earn.landing.title')}
            </h1>
            <p className={styles.tagline}>{t(copy.tagline)}</p>
          </div>
          <p className={styles.description}>
            {getCurrentLocale().formatMessage(
              { id: 'earn.landing.description' },
              {
                hl: (chunks: React.ReactNode[]) => (
                  <span
                    className={styles.highlight}
                    key={String(chunks)}
                  >
                    {chunks}
                  </span>
                ),
              },
            )}{' '}
            <a
              className={styles.knowMore}
              href={CREATOR_FUND_URL}
              onClick={event => {
                event.preventDefault();
                void misc.openExternal(CREATOR_FUND_URL);
              }}
            >
              {t('earn.landing.know_more')}
            </a>
          </p>
        </div>
        <ul className={styles.features}>
          {FEATURES.map(({ key, Icon, title, body }) => (
            <li
              className={styles.feature}
              key={key}
            >
              <span className={styles.featureTitle}>
                <Icon />
                {t(title)}
              </span>
              <span className={styles.featureBody}>{t(body)}</span>
            </li>
          ))}
        </ul>
      </div>
      <div className={styles.ctaArea}>{children}</div>
    </section>
  );
}

/**
 * The landing CTAs: join (or sign in to join) and the read-only terms.
 * @param props.onJoin - join when signed in, sign in to join when signed out.
 */
export function LandingCtas({
  audience,
  onJoin,
}: {
  audience: LandingAudience;
  onJoin: () => void;
}) {
  const [readingTerms, setReadingTerms] = useState(false);
  return (
    <>
      <div className={styles.ctaRow}>
        <Button
          className={styles.heroCta}
          variant="contained"
          color="primary"
          disableRipple
          onClick={onJoin}
        >
          <img
            className={styles.ctaEmoji}
            src={moneyMouth}
            alt=""
            width={18}
            height={18}
          />
          {t(COPY[audience].cta)}
        </Button>
        <Button
          className={styles.heroCta}
          variant="contained"
          color="secondary"
          disableRipple
          onClick={() => {
            void analytics.track('Creator Fund Read Terms', { source: 'landing' });
            setReadingTerms(true);
          }}
        >
          {t('earn.landing.read_terms')}
        </Button>
      </div>
      {readingTerms && (
        <TermsModal
          variant="read"
          onClose={() => setReadingTerms(false)}
        />
      )}
    </>
  );
}

/** Replaces the landing CTAs for a suspended enrollment, which can't rejoin here. */
export function LandingStatus() {
  const [readingTerms, setReadingTerms] = useState(false);
  return (
    <>
      <StatusSnackbar
        tone="danger"
        role="status"
        icon={<WarningIcon />}
        message={t('earn.landing.status.suspended')}
      >
        <SnackbarAction
          emphasis="muted"
          onClick={() => {
            void analytics.track('Creator Fund Read Terms', { source: 'snackbar' });
            setReadingTerms(true);
          }}
        >
          {t('earn.joined.read_terms')}
        </SnackbarAction>
        <SnackbarAction
          onClick={() => {
            void analytics.track('Creator Fund Contact Support', { source: 'snackbar' });
            void misc.openExternal(CONTACT_SUPPORT_URL);
          }}
        >
          {t('earn.joined.contact_support')}
        </SnackbarAction>
      </StatusSnackbar>
      {readingTerms && (
        <TermsModal
          variant="read"
          onClose={() => setReadingTerms(false)}
        />
      )}
    </>
  );
}

/** Replaces the landing CTAs while the program status can't be loaded. */
export function LandingLoadError({ onRetry }: { onRetry: () => void }) {
  return (
    <StatusSnackbar
      tone="neutral"
      role="alert"
      icon={<AlertIcon />}
      message={t('earn.errors.load_status')}
    >
      <SnackbarAction
        emphasis="primary"
        onClick={onRetry}
      >
        {t('earn.errors.retry')}
      </SnackbarAction>
    </StatusSnackbar>
  );
}
