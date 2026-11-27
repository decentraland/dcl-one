import { useEffect, useId, useRef, useState, type ReactNode } from 'react'

import { Button, KebabMenu, ProfilePic, WebTextarea } from '../../../index'
import {
  GeneralLike,
  GeneralLikeFilled,
  GeneralVerified,
  MenusLink
} from '../../../foundations/icons'

import {
  fullDate,
  isImageUrl,
  oldestFirst,
  relativeDate,
  shortAddress,
  type HubPostView
} from './hub-format'
import { HubConfirm, HubLoading, HubRetry } from './SocialHub'
import styles from './CommunityAnnouncements.module.css'

/** One announcement: its author, when it was written (the full date on hover) and its words. */
export function PostMessage({
  post,
  now,
  children
}: {
  post: HubPostView
  now?: number
  children?: ReactNode
}) {
  const name = post.authorName || shortAddress(post.authorAddress)
  return (
    <article id={`post-${post.id}`} className={styles.message} title={fullDate(post.createdAt)}>
      <ProfilePic
        name={name}
        src={isImageUrl(post.authorProfilePictureUrl) ? post.authorProfilePictureUrl : undefined}
        alt=""
        size={40}
      />
      <div className={styles.messageBody}>
        <header>
          <strong>{name}</strong>
          {post.authorHasClaimedName ? <GeneralVerified size={14} aria-label="verified" /> : null}
          <time dateTime={post.createdAt ?? undefined}>{relativeDate(post.createdAt, now)}</time>
        </header>
        <p>{post.content}</p>
        {children}
      </div>
    </article>
  )
}

export type CommunityAnnouncementsProps = {
  /** In any order; they read oldest first. */
  posts: HubPostView[]
  viewer: string | null
  /** The reader owns or moderates the community: they publish, edit their own and delete. */
  canPost: boolean
  /** An owner deletes anyone's announcement; a moderator only their own. */
  isOwner: boolean
  /** The reader may like announcements. */
  canLike: boolean
  pending: boolean
  error?: string
  onRetry: () => void
  hasMore: boolean
  loadingMore: boolean
  onLoadMore: () => void
  /** The link that opens the community on one announcement. */
  postHref: (postId: string) => string
  /** An announcement the visitor followed a link to: shown in view once it loads. */
  focusPostId?: string | null
  onPublish: (content: string) => Promise<void>
  onEdit: (post: HubPostView, content: string) => Promise<void>
  onDelete: (post: HubPostView) => Promise<void>
  onLike: (post: HubPostView) => Promise<void>
  now?: number
}

const errorText = (error: unknown) =>
  error instanceof Error && error.message
    ? error.message
    : 'The post could not be updated. Try again.'

/**
 * The community's announcements. Owners and moderators publish; a failed write keeps what was typed
 * and says why, and nothing is sent twice while a write is on its way.
 */
export function CommunityAnnouncements({
  posts,
  viewer,
  canPost,
  isOwner,
  canLike,
  pending,
  error: readError,
  onRetry,
  hasMore,
  loadingMore,
  onLoadMore,
  postHref,
  focusPostId,
  onPublish,
  onEdit,
  onDelete,
  onLike,
  now
}: CommunityAnnouncementsProps) {
  const [composing, setComposing] = useState(false)
  const [draft, setDraft] = useState('')
  const [editing, setEditing] = useState<HubPostView | null>(null)
  const [deleting, setDeleting] = useState<HubPostView | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const sending = useRef(false)
  const textId = useId()
  const loaded = !pending && !readError

  useEffect(() => {
    if (focusPostId)
      document.getElementById(`post-${focusPostId}`)?.scrollIntoView?.({ block: 'center' })
  }, [focusPostId, posts, readError])

  async function run(write: () => Promise<void>, after: () => void) {
    if (sending.current) return
    sending.current = true
    setBusy(true)
    setError('')
    try {
      await write()
      after()
    } catch (failure) {
      setError(errorText(failure))
    } finally {
      sending.current = false
      setBusy(false)
    }
  }
  const closeForm = () => {
    setDraft('')
    setEditing(null)
    setComposing(false)
  }
  const own = (post: HubPostView) =>
    !!viewer && post.authorAddress.toLowerCase() === viewer.toLowerCase()

  return (
    <section className={styles.root} aria-label="Community posts">
      <header className={styles.head}>
        <h1>Announcements</h1>
        {canPost && !composing ? (
          <Button
            variant="primary"
            size="md"
            onClick={() => {
              setError('')
              setEditing(null)
              setDraft('')
              setComposing(true)
            }}
          >
            New post
          </Button>
        ) : null}
      </header>
      {error && !deleting ? (
        <p className={styles.error} role="alert">
          {error}
        </p>
      ) : null}
      {composing && canPost ? (
        <form
          className={styles.form}
          aria-label={editing ? 'Edit post' : 'New post'}
          onSubmit={(event) => {
            event.preventDefault()
            const content = draft
            void run(() => (editing ? onEdit(editing, content) : onPublish(content)), closeForm)
          }}
        >
          <label className={styles.field} htmlFor={textId}>
            Post text
          </label>
          <WebTextarea
            id={textId}
            className={styles.textarea}
            autoFocus
            required
            value={draft}
            disabled={busy}
            onChange={(event) => setDraft(event.target.value)}
          />
          <div className={styles.actions}>
            <Button type="submit" variant="primary" size="md" disabled={busy || !draft.trim()}>
              {busy ? 'Saving…' : editing ? 'Save changes' : 'Publish post'}
            </Button>
            <Button
              type="button"
              variant="outline"
              size="md"
              disabled={busy}
              onClick={() => {
                closeForm()
                setError('')
              }}
            >
              Cancel
            </Button>
          </div>
        </form>
      ) : null}
      {!canPost ? (
        <p className={styles.muted}>Only owners and moderators can publish announcements.</p>
      ) : null}
      {pending ? <HubLoading label="Loading announcements" /> : null}
      {readError ? <HubRetry message={readError} onRetry={onRetry} /> : null}
      {hasMore && !readError ? (
        <Button variant="outline" size="sm" disabled={busy || loadingMore} onClick={onLoadMore}>
          {loadingMore ? 'Loading older posts…' : 'Load older posts'}
        </Button>
      ) : null}
      <div className={styles.thread} aria-label="Posts">
        {focusPostId && loaded && !posts.some((post) => post.id === focusPostId) ? (
          <p role="status" className={styles.muted}>
            {hasMore
              ? 'Load older posts to find this announcement.'
              : 'This announcement is unavailable.'}
          </p>
        ) : null}
        {loaded && !posts.length ? <p className={styles.muted}>No announcements yet.</p> : null}
        {oldestFirst(posts).map((post) => (
          <PostMessage key={post.id} post={post} now={now}>
            <div className={styles.postActions}>
              <a className={styles.postLink} href={postHref(post.id)}>
                <MenusLink size={16} aria-hidden="true" />
                Post link
              </a>
              <span aria-label="Likes">
                {post.likesCount} {post.likesCount === 1 ? 'like' : 'likes'}
              </span>
              {canLike ? (
                <Button
                  variant="outline"
                  size="sm"
                  aria-pressed={!!post.isLikedByUser}
                  disabled={busy}
                  leadingIcon={
                    post.isLikedByUser ? (
                      <GeneralLikeFilled size={16} aria-hidden="true" />
                    ) : (
                      <GeneralLike size={16} aria-hidden="true" />
                    )
                  }
                  onClick={() =>
                    void run(
                      () => onLike(post),
                      () => {}
                    )
                  }
                >
                  {post.isLikedByUser ? 'Unlike' : 'Like'}
                </Button>
              ) : null}
              {canPost && !composing && (isOwner || own(post)) ? (
                <KebabMenu
                  label="Post actions"
                  size="sm"
                  items={(close) => [
                    ...(own(post)
                      ? [
                          {
                            label: 'Edit post',
                            onClick: () => {
                              close()
                              if (busy) return
                              setError('')
                              setEditing(post)
                              setDraft(post.content)
                              setComposing(true)
                            }
                          }
                        ]
                      : []),
                    {
                      label: 'Delete post',
                      danger: true,
                      onClick: () => {
                        close()
                        if (busy) return
                        setError('')
                        setDeleting(post)
                      }
                    }
                  ]}
                />
              ) : null}
            </div>
          </PostMessage>
        ))}
      </div>
      {deleting ? (
        <HubConfirm
          title="Delete post?"
          body="This announcement will be removed for everyone."
          error={error || undefined}
          danger
          busy={busy}
          confirmLabel={busy ? 'Deleting…' : 'Delete post'}
          cancelLabel="Keep post"
          onConfirm={() =>
            void run(
              () => onDelete(deleting),
              () => setDeleting(null)
            )
          }
          onCancel={() => {
            if (busy) return
            setDeleting(null)
            setError('')
          }}
        />
      ) : null}
    </section>
  )
}
