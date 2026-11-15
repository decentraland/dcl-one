import type { ReactNode } from 'react';
import EmptyLands from '/assets/images/empty-lands.svg';
import EmptySignedOut from '/assets/images/empty-signed-out.svg';
import EmptyWorlds from '/assets/images/empty-worlds.svg';

import { t } from '/@/modules/store/translation/utils';

import { Button } from '../../Button';
import { ExternalLink } from '../../ExternalLink';
import { InfoTip } from '../InfoTip';

import styles from './EmptyState.module.css';

type EmptyStateIcon = 'wave' | 'world' | 'land';

type EmptyStateAction = {
  label: string;
  variant: 'primary' | 'secondary' | 'tertiary';
  onClick: () => void;
};

type EmptyStateInfo = { label: string; text: string; readMoreUrl: string };

const EMPTY_STATE_ICONS = { wave: EmptySignedOut, world: EmptyWorlds, land: EmptyLands };

const BUTTON_BY_VARIANT = {
  primary: { variant: 'contained', color: 'primary' },
  secondary: { variant: 'contained', color: 'secondary' },
  tertiary: { variant: 'outlined', color: 'secondary' },
} as const;

type Props = {
  icon?: EmptyStateIcon;
  title: string;
  info?: EmptyStateInfo;
  description?: ReactNode;
  action?: EmptyStateAction;
};

/** The Manage page's empty, signed-out and error views. */
export function EmptyState({ icon, title, info, description, action }: Props) {
  return (
    <section className={styles.root}>
      {icon && (
        <img
          className={styles.illustration}
          src={EMPTY_STATE_ICONS[icon]}
          alt=""
        />
      )}
      <div className={styles.titleRow}>
        <h2 className={styles.title}>{title}</h2>
        {info && (
          <InfoTip label={info.label}>
            {info.text}{' '}
            <ExternalLink
              className={styles.readMore}
              href={info.readMoreUrl}
            >
              {t('manage.read_more')}
            </ExternalLink>
          </InfoTip>
        )}
      </div>
      {description && <p className={styles.body}>{description}</p>}
      {action && (
        <Button
          size="large"
          {...BUTTON_BY_VARIANT[action.variant]}
          onClick={action.onClick}
        >
          {action.label}
        </Button>
      )}
    </section>
  );
}
