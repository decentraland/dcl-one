import { ForumOutlinedIcon as ForumIcon, OpenInNewIcon } from '@/features/creators/components/Icons'
import { Button } from '@/features/creators/components/Button'
import { Modal } from '@/features/creators/components/Modal'
import { useTranslation } from '@/data/lib/creators/intl'
import { type Collection } from '@/data/lib/creators/collections'
import { openForumPost } from '@/data/lib/creators/forumPost'
import styles from './ForumVerdictModal.module.css'
import shared from '@/features/creators/styles/shared.module.css'

type Props = {
  collection: Collection & { forumLink: string }
  onClose: () => void
}

/** After a rejection or a disable: the creator learns why on the collection's forum post. */
export function ForumVerdictModal({ collection, onClose }: Props) {
  const { t } = useTranslation()
  const title = t('item_editor.review.verdict.title')
  return (
    <Modal title={title} onClose={onClose} hideTitle testId="review-verdict">
      <div className={styles.wrap}>
        <div className={styles.icon} aria-hidden>
          <ForumIcon />
        </div>
        <h3 className={styles.heading}>{title}</h3>
        <p className={styles.text}>{t('item_editor.review.verdict.body', { collection: collection.name })}</p>
        <div className={shared.modalActions}>
          <Button type="button" variant="secondary" data-testid="review-verdict-done" onClick={onClose}>
            {t('item_editor.review.verdict.done')}
          </Button>
          <Button
            type="button"
            data-testid="review-verdict-forum-link"
            onClick={() => openForumPost(collection, 'review_verdict')}
          >
            {t('item_editor.review.verdict.open_forum')}
            <OpenInNewIcon fontSize="small" aria-hidden />
          </Button>
        </div>
      </div>
    </Modal>
  )
}
