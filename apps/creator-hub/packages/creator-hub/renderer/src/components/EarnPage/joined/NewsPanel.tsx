import { useId } from 'react';
import cx from 'classnames';

import type { NewsItem } from '/shared/types/creator-fund';

import { useSelector } from '#store';
import { t } from '/@/modules/store/translation/utils';

import { formatDate } from './format';
import styles from './NewsPanel.module.css';

/** The latest Creator Fund news, plain text. */
export function NewsPanel({ items, className }: { items: NewsItem[]; className?: string }) {
  const headingId = useId();
  const locale = useSelector(state => state.translation.locale);
  return (
    <aside
      aria-labelledby={headingId}
      className={cx(styles.panel, className)}
    >
      <h2
        className={styles.heading}
        id={headingId}
      >
        {t('earn.joined.news.title')}
      </h2>
      {items.map(item => (
        <article
          key={item.id}
          className={styles.entry}
        >
          <time
            className={styles.entryDate}
            dateTime={item.published_at}
          >
            {formatDate(item.published_at, locale)}
          </time>
          <h3 className={styles.entryTitle}>{item.title}</h3>
          <p className={styles.entryBody}>{item.body}</p>
        </article>
      ))}
    </aside>
  );
}
