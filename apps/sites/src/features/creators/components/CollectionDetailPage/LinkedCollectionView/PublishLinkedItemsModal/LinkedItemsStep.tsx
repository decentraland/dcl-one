import { useState } from 'react'
import { ChevronLeftIcon, ChevronRightIcon } from '../../../Icons'
import type { ContractNetwork } from '@dcl/schemas'
import { useTranslation } from '@/data/lib/creators/intl'
import { getContentsStorageUrl } from '@/data/lib/creators/builder'
import type { Collection } from '@/data/lib/creators/collections'
import type { Item } from '@/data/lib/creators/items'
import { getMappingLabels } from '@/data/lib/creators/mappings'
import { Button } from '../../../Button'
import { Checkbox } from '../../../Checkbox'
import { CategoryIcon } from '../../../ItemIcons'
import { ItemThumbnail } from '../../../ItemThumbnail'
import { cx } from '@/data/lib/creators/cx'
import publishRow from './LinkedPublishRow.module.css'
import publishModal from '../../PublishCollectionModal/PublishCollectionModal.module.css'

type Props = {
  collection: Collection
  itemsToPublish: Item[]
  itemsToPush: Item[]
  /** The wallet prompt for the slots cheque is open. */
  isSigning: boolean
  signError: boolean
  onBack: () => void
  onConfirm: () => void
}

/** Step 2: the items about to be submitted, each with the token mapping that unlocks it; Next signs the cheque. */
export function LinkedItemsStep({
  collection,
  itemsToPublish,
  itemsToPush,
  isSigning,
  signError,
  onBack,
  onConfirm
}: Props) {
  const { t } = useTranslation()
  const [accepted, setAccepted] = useState(false)
  const network = collection.linkedContractNetwork as ContractNetwork | undefined
  const address = collection.linkedContractAddress

  const renderRows = (items: Item[], testId: string) =>
    items.map(item => {
      const thumbnailHash = item.contents[item.thumbnail]
      const mapping = getMappingLabels(item.mappings, network, address)
        .map(label => t(label.id, label.values))
        .join(' · ')
      return (
        <div className={cx(publishRow.columns, publishRow.row)} key={item.id} data-testid={testId}>
          <span className={publishRow.name}>
            <span className={publishModal.thumb}>
              <ItemThumbnail src={thumbnailHash ? getContentsStorageUrl(thumbnailHash) : null} rarity={item.rarity} />
            </span>
            <span title={item.name}>{item.name}</span>
          </span>
          <span>{item.data.category ? <CategoryIcon category={item.data.category} withLabel /> : '—'}</span>
          <span className={publishRow.mapping} title={mapping} data-testid={`${testId}-mapping`}>
            {mapping}
          </span>
        </div>
      )
    })

  return (
    <div className={publishModal.step} data-testid="publish-linked-items-step">
      <h3 className={publishModal.heading}>{t('publish_linked_modal.items_step.title')}</h3>
      <p className={publishModal.text}>{t('publish_linked_modal.items_step.description')}</p>
      {itemsToPublish.length > 0 && (
        <div className={publishModal.table}>
          <div className={cx(publishModal.tableHeader, publishRow.columns)}>
            <span>{t('publish_linked_modal.items_step.columns.item', { count: itemsToPublish.length })}</span>
            <span>{t('publish_collection_modal.items_step.columns.category')}</span>
            <span>{t('publish_linked_modal.items_step.columns.linked_to')}</span>
          </div>
          <div className={publishModal.tableBody} data-testid="publish-linked-items-list">
            {renderRows(itemsToPublish, 'publish-linked-item')}
          </div>
        </div>
      )}
      {itemsToPush.length > 0 && (
        <div className={publishModal.table}>
          <div className={cx(publishModal.tableHeader, publishRow.columns)}>
            <span>{t('publish_linked_modal.items_step.columns.changed', { count: itemsToPush.length })}</span>
            <span>{t('publish_collection_modal.items_step.columns.category')}</span>
            <span>{t('publish_linked_modal.items_step.columns.linked_to')}</span>
          </div>
          <div className={publishModal.tableBody} data-testid="publish-linked-push-list">
            {renderRows(itemsToPush, 'publish-linked-push-item')}
          </div>
        </div>
      )}
      <Checkbox checked={accepted} disabled={isSigning} onChange={setAccepted} testId="publish-linked-items-accept">
        {t('publish_linked_modal.items_step.checkbox')}
      </Checkbox>
      {signError && (
        <p className={publishModal.errorText} data-testid="publish-linked-sign-error">
          {t('publish_linked_modal.items_step.sign_error')}
        </p>
      )}
      <div className={publishModal.footer}>
        <Button type="button" variant="secondary" disabled={isSigning} data-testid="publish-back" onClick={onBack}>
          <ChevronLeftIcon fontSize="small" />
          {t('publish_collection_modal.back')}
        </Button>
        <Button
          type="button"
          disabled={!accepted}
          loading={isSigning}
          data-testid="publish-linked-items-confirm"
          onClick={onConfirm}
        >
          {t(
            itemsToPublish.length > 0
              ? 'publish_linked_modal.items_step.sign'
              : 'publish_collection_modal.items_step.confirm'
          )}
          <ChevronRightIcon fontSize="small" />
        </Button>
      </div>
    </div>
  )
}
