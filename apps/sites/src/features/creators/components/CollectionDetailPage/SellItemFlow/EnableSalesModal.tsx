import enableSalesArt from '~/assets/enable-sales.png'
import { ConfirmModal } from '~/components/ConfirmModal'
import { useTranslation } from '~/intl'

/** Why sales must be enabled: a collection's first sale, or one already selling on an older marketplace. */
export type EnableSalesReason = 'first-sale' | 'marketplace-upgrade'

type Props = {
  reason: EnableSalesReason
  /** The transaction is being signed by a custodial wallet, with no prompt to wait for. */
  busy: boolean
  onCancel: () => void
  onConfirm: () => void
}

/** The current off-chain marketplace must be allowed to mint the collection's items before it can sell them. */
export function EnableSalesModal({ reason, busy, onCancel, onConfirm }: Props) {
  const { t } = useTranslation()
  return (
    <ConfirmModal
      title={t('sell_item_modal.enable_sales.title')}
      description={t(
        reason === 'marketplace-upgrade'
          ? 'sell_item_modal.enable_sales.upgrade_description'
          : 'sell_item_modal.enable_sales.description'
      )}
      art={{ src: enableSalesArt }}
      busy={busy}
      onClose={onCancel}
      cancel={{ label: t('sell_item_modal.cancel'), onClick: onCancel, testId: 'enable-sales-cancel' }}
      confirm={{ label: t('sell_item_modal.enable_sales.confirm'), onClick: onConfirm, testId: 'enable-sales-confirm' }}
      testId="enable-sales-modal"
    />
  )
}
