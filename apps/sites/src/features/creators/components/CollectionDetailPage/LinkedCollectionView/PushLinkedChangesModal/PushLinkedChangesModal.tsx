import { useId, useState } from 'react'
import { SyncIcon } from '~/components/Icons'
import { useIntl } from 'react-intl'
import { config } from '~/config'
import { useTranslation } from '~/intl'
import { usePushLinkedChanges } from '~/hooks/useLinkedPublish'
import { useProfile } from '~/hooks/useProfile'
import { type Session } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { EMAIL_MAX_LENGTH, isValidEmail } from '~/lib/email'
import { type Item } from '~/lib/items'
import { openExternal } from '~/lib/navigation'
import { useNotifications } from '~/lib/notifications'
import { Button } from '~/components/Button'
import { Checkbox } from '~/components/Checkbox'
import { ConfirmModal } from '~/components/ConfirmModal'
import { Modal } from '~/components/Modal'
import { ProgressModal } from '~/components/ProgressModal'
import styles from '~/components/CollectionDetailPage/PublishCollectionModal/PublishCollectionModal.module.css'

type Props = {
  collection: Collection
  session: Session
  items: Item[]
  onClose: () => void
  onPushed: () => void
}

/** Sends changed, already-approved linked items for review again: a confirmation, the terms, then one request per item. */
export function PushLinkedChangesModal({ collection, session, items, onClose, onPushed }: Props) {
  const { t } = useTranslation()
  const intl = useIntl()
  const emailErrorId = useId()
  const showToast = useNotifications(state => state.showToast)
  const profile = useProfile(session.address)
  const [view, setView] = useState<'confirm' | 'terms'>('confirm')
  const [email, setEmail] = useState<string | null>(null)
  const [emailBlurred, setEmailBlurred] = useState(false)
  const [accepted, setAccepted] = useState(false)
  const [progress, setProgress] = useState({ done: 0, total: items.length })
  const push = usePushLinkedChanges(session, collection)

  const currentEmail = email ?? profile.data?.email?.trim() ?? ''
  const emailValid = isValidEmail(currentEmail)
  const showEmailError = emailBlurred && !!currentEmail && !emailValid

  const openLink = (url: string) => (event: React.MouseEvent) => {
    event.preventDefault()
    openExternal(url)
  }

  function submit() {
    // A retry after a partial failure counts from zero again.
    setProgress({ done: 0, total: items.length })
    push.mutate(
      { items, email: currentEmail, onProgress: (done, total) => setProgress({ done, total }) },
      {
        onSuccess: result => {
          onPushed()
          onClose()
          if (result.failed.length > 0) {
            showToast(t('push_linked_modal.partial', { count: result.failed.length }), { type: 'error' })
          } else {
            showToast(t('push_linked_modal.success', { count: result.pushed.length }))
          }
        }
      }
    )
  }

  if (push.isPending) {
    return (
      <ProgressModal
        title={t('push_linked_modal.title')}
        heading={t('push_linked_modal.running.heading')}
        description={t('push_linked_modal.running.description')}
        label={t('push_linked_modal.running.label')}
        done={progress.done}
        total={progress.total}
        testId="push-linked-progress"
      />
    )
  }

  if (view === 'confirm') {
    return (
      <ConfirmModal
        icon={<SyncIcon />}
        title={t('push_linked_modal.title')}
        description={t('push_linked_modal.description', { count: items.length })}
        onClose={onClose}
        cancel={{
          label: t('collection_detail_page.push_changes.cancel'),
          onClick: onClose,
          testId: 'push-linked-cancel'
        }}
        confirm={{
          label: t('collection_detail_page.push_changes.confirm'),
          onClick: () => setView('terms'),
          testId: 'push-linked-continue'
        }}
        testId="push-linked-modal"
      />
    )
  }

  return (
    <Modal title={t('push_linked_modal.title')} size="large" onClose={onClose} testId="push-linked-terms-modal">
      <div className={styles.main}>
        <form
          className={styles.step}
          onSubmit={event => (event.preventDefault(), submit())}
          data-testid="push-linked-terms-step"
        >
          <h3 className={styles.heading}>{t('push_linked_modal.terms.title')}</h3>
          <p className={styles.text}>{t('push_linked_modal.terms.description')}</p>
          <div className={styles.inputWrapper}>
            <label className={styles.emailField}>
              {t('publish_collection_modal.name_step.email_label')}
              <input
                className={styles.emailInput}
                type="text"
                inputMode="email"
                spellCheck={false}
                autoComplete="email"
                maxLength={EMAIL_MAX_LENGTH}
                aria-required
                value={currentEmail}
                placeholder={t('publish_collection_modal.name_step.email_placeholder')}
                aria-invalid={showEmailError}
                aria-describedby={showEmailError ? emailErrorId : undefined}
                data-invalid={showEmailError ? true : undefined}
                data-testid="push-linked-email"
                onChange={event => setEmail(event.target.value)}
                onFocus={() => setEmailBlurred(false)}
                onBlur={() => setEmailBlurred(true)}
              />
              {showEmailError && (
                <p className={styles.errorText} id={emailErrorId} role="alert">
                  {t('publish_collection_modal.name_step.invalid_email')}
                </p>
              )}
            </label>
          </div>
          <Checkbox checked={accepted} onChange={setAccepted} testId="push-linked-terms-accept">
            {intl.formatMessage(
              { id: 'push_linked_modal.terms.checkbox' },
              {
                terms: chunks => (
                  <a href={config.get('TERMS_OF_USE_URL')} onClick={openLink(config.get('TERMS_OF_USE_URL'))}>
                    {chunks}
                  </a>
                ),
                policy: chunks => (
                  <a href={config.get('CONTENT_POLICY_URL')} onClick={openLink(config.get('CONTENT_POLICY_URL'))}>
                    {chunks}
                  </a>
                )
              }
            )}
          </Checkbox>
          {push.isError && (
            <p className={styles.errorText} data-testid="push-linked-error">
              {t('push_linked_modal.error')}
            </p>
          )}
          <div className={styles.footer}>
            <Button type="button" variant="secondary" data-testid="push-linked-back" onClick={() => setView('confirm')}>
              {t('publish_collection_modal.back')}
            </Button>
            <Button type="submit" disabled={!accepted || !emailValid} data-testid="push-linked-submit">
              {t('push_linked_modal.submit')}
            </Button>
          </div>
        </form>
      </div>
    </Modal>
  )
}
