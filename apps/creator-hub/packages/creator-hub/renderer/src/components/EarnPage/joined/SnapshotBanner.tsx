import { t } from '/@/modules/store/translation/utils';

import MarkdownRenderer from '../../Modals/AppSettings/MarkdownRenderer/MarkdownRenderer';
import { openMarkdownLink } from '../markdownLinks';

import styles from './SnapshotBanner.module.css';

/** Whether the leaderboard fetches and shows the banner; off for v0, whose API has no banner. */
export const SNAPSHOT_BANNER_ENABLED = false;

/** The global Creator Fund announcement: light markdown, links opened in the system browser. */
export function SnapshotBanner({ markdown }: { markdown: string }) {
  return (
    <div className={styles.banner}>
      <div
        className={styles.bannerText}
        role="region"
        aria-label={t('earn.joined.banner.region')}
        onClick={openMarkdownLink}
      >
        <MarkdownRenderer content={markdown} />
      </div>
    </div>
  );
}
