import { useState } from 'react'
import { useTranslation } from '@/data/lib/creators/intl'
import { Button } from '../Button'
import { NoteField } from '../NoteField'
import { usePostComment } from '../../hooks/useCollectionEvents'
import type { Collection } from '@/data/lib/creators/collections'
import { COMMENT_MAX_LENGTH, type ReviewParty } from '@/data/lib/creators/events'
import styles from './CollectionActivityModal.module.css'

// The limit rarely matters, so the length only shows once it is close.
const COUNT_FROM = COMMENT_MAX_LENGTH * 0.8

type Props = {
  collection: Collection
  address: string
  /** The side of the review the signed-in wallet writes as. */
  role: ReviewParty
}

/** The box creators and curators write to each other from, on top of the timeline their messages land in. */
export function CommentComposer({ collection, address, role }: Props) {
  const { t } = useTranslation()
  const post = usePostComment(address)
  const [message, setMessage] = useState('')
  const trimmed = message.trim()

  function submit(event: React.FormEvent) {
    event.preventDefault()
    if (!trimmed || post.isPending) return
    post.mutate({ collection, message: trimmed, role }, { onSuccess: () => setMessage('') })
  }

  return (
    <form className={styles.composer} onSubmit={submit} data-testid="activity-composer">
      <NoteField
        label={t(`activity_modal.comment.label_${role}`)}
        value={message}
        onChange={setMessage}
        maxLength={COMMENT_MAX_LENGTH}
        placeholder={t(`activity_modal.comment.placeholder_${role}`)}
        disabled={post.isPending}
        hideCount
        fixedSize
        testId="activity-comment-input"
      />
      {post.isError && (
        <p className={styles.composerError} data-testid="activity-comment-error">
          {t('activity_modal.comment.error')}
        </p>
      )}
      <div className={styles.composerActions}>
        {message.length >= COUNT_FROM && (
          <span className={styles.composerCount} data-testid="activity-comment-count">
            {message.length}/{COMMENT_MAX_LENGTH}
          </span>
        )}
        <Button
          className={styles.composerSend}
          type="submit"
          size="sm"
          loading={post.isPending}
          disabled={!trimmed}
          data-testid="activity-comment-send"
        >
          {t('activity_modal.comment.send')}
        </Button>
      </div>
    </form>
  )
}
