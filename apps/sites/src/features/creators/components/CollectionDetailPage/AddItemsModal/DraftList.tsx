import {
  EmoteIcon,
  SmartIcon,
  TrashIcon,
  WearableIcon,
  CheckIcon,
  ErrorRoundedIcon as ErrorIcon
} from '@/features/creators/components/Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { isThumbnailGenerating } from '@/data/lib/creators/draftPreview'
import { ItemType } from '@/data/lib/creators/items'
import { type ItemDraft } from './AddItemsModal.state'
import styles from './AddItemsModal.module.css'
import { ItemThumbnail } from '@/features/creators/components/ItemThumbnail'

type Props = {
  drafts: ItemDraft[]
  selectedId: string | null
  onSelect: (id: string) => void
  onRemove: (id: string) => void
}

/** Sidebar with one card per file in the batch; only this pane scrolls on long batches. */
export function DraftList({ drafts, selectedId, onSelect, onRemove }: Props) {
  const { t } = useTranslation()

  return (
    <aside className={styles.sidebar} data-testid="draft-list">
      {drafts.map(draft => (
        <button
          className={styles.draftCard}
          key={draft.id}
          type="button"
          data-testid={`draft-${draft.id}`}
          data-file-name={draft.fileName}
          data-status={draft.status}
          data-selected={draft.id === selectedId || undefined}
          data-failed={draft.status === 'failed' || undefined}
          data-checked={draft.checked || undefined}
          onClick={() => onSelect(draft.id)}
        >
          <span className={styles.draftThumbWrap}>
            <ItemThumbnail
              src={draft.thumbnail}
              rarity={draft.rarity}
              testId={`draft-thumbnail-${draft.id}`}
              loading={draft.status === 'processing' || (draft.status === 'ready' && isThumbnailGenerating(draft))}
            />
            {draft.status !== 'failed' && (
              <span
                className={styles.draftCheck}
                data-checked={draft.checked || undefined}
                data-testid={`draft-check-${draft.id}`}
              >
                {draft.checked && <CheckIcon />}
              </span>
            )}
          </span>
          <span className={styles.draftInfo}>
            <span className={styles.draftName}>{draft.name || draft.fileName}</span>
            {draft.status === 'failed' ? (
              <span className={styles.draftError} data-testid={`draft-error-${draft.id}`}>
                <ErrorIcon />
                {t('add_items_modal.file_error_label')}
              </span>
            ) : draft.status === 'processing' || !draft.type ? (
              <span className={styles.draftType}>{t('add_items_modal.processing')}</span>
            ) : (
              <span
                className={styles.draftType}
                data-testid={`draft-type-${draft.id}`}
                data-smart={draft.isSmart || undefined}
              >
                {draft.type === ItemType.EMOTE ? <EmoteIcon /> : draft.isSmart ? <SmartIcon /> : <WearableIcon />}
                {t(draft.isSmart ? 'add_items_modal.type.smart_wearable' : `add_items_modal.type.${draft.type}`)}
              </span>
            )}
          </span>
          <span
            className={styles.deleteButton}
            role="button"
            aria-label={t('add_items_modal.delete')}
            title={t('add_items_modal.delete')}
            data-testid={`draft-remove-${draft.id}`}
            onClick={event => {
              event.stopPropagation()
              onRemove(draft.id)
            }}
          >
            <TrashIcon />
          </span>
        </button>
      ))}
    </aside>
  )
}
