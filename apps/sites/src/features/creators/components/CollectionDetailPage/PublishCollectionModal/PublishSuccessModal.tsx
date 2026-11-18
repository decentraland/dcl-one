import { CheckIcon } from '@/features/creators/components/Icons'
import { Modal } from '@/features/creators/components/Modal'
import { Button } from '@/features/creators/components/Button'
import { useTranslation } from '@/data/lib/creators/intl'
import successArt from '@/features/creators/assets/publish-success.png'
import styles from './PublishSuccessModal.module.css'

type Props = {
  onDone: () => void
}

/** Shown once the publish transaction is sent: the collection is now with the curators. */
export function PublishSuccessModal({ onDone }: Props) {
  const { t } = useTranslation()
  return (
    <Modal
      title={t('publish_collection_modal.success.title')}
      onClose={onDone}
      hideTitle
      testId="publish-success-modal"
    >
      <div className={styles.wrap}>
        <img className={styles.art} src={successArt} alt="" />
        <p className={styles.text} data-testid="publish-success-description">
          {t('publish_collection_modal.success.description')}
        </p>
        <Button type="button" data-testid="publish-success-done" onClick={onDone}>
          {t('publish_collection_modal.success.done')}
          <CheckIcon fontSize="small" />
        </Button>
      </div>
    </Modal>
  )
}
