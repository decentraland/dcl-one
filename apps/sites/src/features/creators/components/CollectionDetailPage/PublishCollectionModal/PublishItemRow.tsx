import { useState } from 'react'
import { useTranslation } from '@/data/lib/creators/intl'
import { getContentsStorageUrl } from '@/data/lib/creators/builder'
import { ItemThumbnail } from '../../ItemThumbnail'
import { ITEM_NAME_MAX_LENGTH, isValidItemName } from '@/data/lib/creators/itemFactory'
import { getItemBodyShapeType, type Item } from '@/data/lib/creators/items'
import { DEFAULT_RARITY, isRarity, type RarityName } from '@/data/lib/creators/rarities'
import { TrashIcon, CheckIcon, CloseIcon, EditIcon } from '../../Icons'
import { BodyShapeIcon, CategoryIcon } from '../../ItemIcons'
import { RarityPill } from '../../RarityPill'
import { RaritySelect } from '../../RaritySelect'
import { cx } from '@/data/lib/creators/cx'
import styles from './PublishItemRow.module.css'
import publishModal from './PublishCollectionModal.module.css'
import shared from '../../../styles/shared.module.css'

type Props = {
  item: Item
  isEditing: boolean
  isSaving: boolean
  saveError: boolean
  /** Replaces the generic save error, for a failure the creator can act on (an item over its size limit). */
  saveErrorMessage?: string
  /** False when no row can act, so the table drops the actions column. */
  withActions: boolean
  canEdit: boolean
  canDelete: boolean
  /** Other rows lock while one is being edited. */
  locked: boolean
  /** Data URL of a thumbnail picked during this edit, shown instead of the saved one. */
  pendingThumbnail: string | null
  onEdit: () => void
  onEditThumbnail: () => void
  onCancelEdit: () => void
  onSave: (changes: { name: string; rarity: string }) => void
  onDelete: () => void
}

/** One item of the review table: read-only, or inline-editing its name and rarity. */
export function PublishItemRow({
  item,
  isEditing,
  isSaving,
  saveError,
  saveErrorMessage,
  withActions,
  canEdit,
  canDelete,
  locked,
  pendingThumbnail,
  onEdit,
  onEditThumbnail,
  onCancelEdit,
  onSave,
  onDelete
}: Props) {
  const { t } = useTranslation()
  const [name, setName] = useState(item.name)
  const [rarity, setRarity] = useState<RarityName>(isRarity(item.rarity) ? item.rarity : DEFAULT_RARITY)

  const thumbnailHash = item.contents[item.thumbnail]
  const thumbnailUrl = pendingThumbnail ?? (thumbnailHash ? getContentsStorageUrl(thumbnailHash) : null)
  const bodyShape = getItemBodyShapeType(item)
  const nameValid = isValidItemName(name)

  function startEditing() {
    setName(item.name)
    setRarity(isRarity(item.rarity) ? item.rarity : DEFAULT_RARITY)
    onEdit()
  }

  function save() {
    if (!nameValid || isSaving) return
    onSave({ name: name.trim(), rarity })
  }

  return (
    <div
      className={cx(styles.columns, styles.row)}
      data-testid="publish-item-row"
      data-item-id={item.id}
      data-editing={isEditing || undefined}
      data-no-actions={!withActions || undefined}
    >
      <div className={styles.nameCell}>
        {isEditing ? (
          <button
            className={styles.thumbButton}
            type="button"
            title={t('publish_collection_modal.items_step.edit_thumbnail')}
            aria-label={t('publish_collection_modal.items_step.edit_thumbnail')}
            disabled={isSaving}
            data-testid="publish-item-edit-thumbnail"
            onClick={onEditThumbnail}
          >
            <span className={publishModal.thumb}>
              <ItemThumbnail src={thumbnailUrl} rarity={item.rarity} testId="publish-item" />
            </span>
            <span className={styles.thumbBadge} aria-hidden>
              <EditIcon />
            </span>
          </button>
        ) : (
          <span className={publishModal.thumb}>
            <ItemThumbnail src={thumbnailUrl} rarity={item.rarity} testId="publish-item" />
          </span>
        )}
        {isEditing ? (
          <input
            className={styles.nameInput}
            value={name}
            maxLength={ITEM_NAME_MAX_LENGTH}
            disabled={isSaving}
            autoFocus
            aria-label={t('publish_collection_modal.items_step.name_placeholder')}
            placeholder={t('publish_collection_modal.items_step.name_placeholder')}
            data-invalid={name && !nameValid ? true : undefined}
            data-testid="publish-item-name-input"
            onChange={event => setName(event.target.value)}
            onKeyDown={event => {
              if (event.key === 'Enter') {
                event.preventDefault()
                save()
              }
            }}
          />
        ) : (
          <span className={styles.name} title={item.name} data-testid="publish-item-name">
            {item.name}
          </span>
        )}
      </div>
      <span data-testid="publish-item-body-type">
        {bodyShape ? <BodyShapeIcon bodyShape={bodyShape} withLabel /> : '—'}
      </span>
      <span data-testid="publish-item-category">
        {item.data.category ? <CategoryIcon category={item.data.category} withLabel /> : '—'}
      </span>
      <span data-testid="publish-item-rarity">
        {isEditing ? (
          <div className={styles.selectCell}>
            <RaritySelect value={rarity} onChange={setRarity} />
          </div>
        ) : (
          item.rarity && <RarityPill rarity={item.rarity} />
        )}
      </span>
      {withActions && (
        <div className={styles.actions} data-testid="publish-item-actions">
          {isEditing ? (
            <>
              <button
                className={styles.iconButton}
                type="button"
                data-variant="cancel"
                title={t('publish_collection_modal.items_step.cancel_edit')}
                aria-label={t('publish_collection_modal.items_step.cancel_edit')}
                disabled={isSaving}
                data-testid="publish-item-cancel"
                onClick={onCancelEdit}
              >
                <CloseIcon />
              </button>
              <button
                className={styles.iconButton}
                type="button"
                data-variant="save"
                title={t('publish_collection_modal.items_step.save')}
                aria-label={t('publish_collection_modal.items_step.save')}
                disabled={!nameValid || isSaving}
                aria-busy={isSaving || undefined}
                data-testid="publish-item-save"
                onClick={save}
              >
                {isSaving ? <span className={shared.spinner} aria-hidden /> : <CheckIcon />}
              </button>
            </>
          ) : (
            <>
              {canEdit && (
                <button
                  className={styles.iconButton}
                  type="button"
                  title={t('publish_collection_modal.items_step.edit')}
                  aria-label={t('publish_collection_modal.items_step.edit')}
                  disabled={locked}
                  data-testid="publish-item-edit"
                  onClick={startEditing}
                >
                  <EditIcon />
                </button>
              )}
              {canDelete && (
                <button
                  className={styles.iconButton}
                  type="button"
                  title={t('publish_collection_modal.items_step.delete')}
                  aria-label={t('publish_collection_modal.items_step.delete')}
                  disabled={locked}
                  data-testid="publish-item-delete"
                  onClick={onDelete}
                >
                  <TrashIcon />
                </button>
              )}
            </>
          )}
        </div>
      )}
      {isEditing && saveError && (
        <p className={styles.rowError} data-testid="publish-item-error">
          {saveErrorMessage ?? t('publish_collection_modal.items_step.save_error')}
        </p>
      )}
    </div>
  )
}
