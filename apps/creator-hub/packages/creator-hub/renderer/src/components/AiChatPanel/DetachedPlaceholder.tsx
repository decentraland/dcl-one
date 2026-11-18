import type { CSSProperties } from 'react';
import { OpenInNewIcon } from '@ui/atoms/icons-material';
import Button from '@ui/atoms/Button';

import { t } from '/@/modules/store/translation/utils';

import styles from './component.module.css';

// Shown in the editor in place of the inline panel while the chat is popped out into its
// own window (#1504) — tells the user where the chat went and offers to dock it back.
export function DetachedPlaceholder({ onDock, width }: { onDock: () => void; width: number }) {
  return (
    <div
      className={styles.panel}
      style={{ '--panel-width': `${width}px` } as CSSProperties}
      aria-label="ai-chat-detached"
    >
      <div className={styles.panelHeader}>
        <div className={styles.headerTitle}>{t('editor.ai.title')}</div>
      </div>
      <div className={styles.emptyState}>
        <div>{t('editor.ai.detached_message')}</div>
        <div>
          <Button
            variant="text"
            tone="neutral"
            size="sm"
            className={styles.textButton}
            startIcon={<OpenInNewIcon fontSize="small" />}
            onClick={onDock}
          >
            {t('editor.ai.detached_dock')}
          </Button>
        </div>
      </div>
    </div>
  );
}
