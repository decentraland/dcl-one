// The community card's ANNOUNCEMENTS tab: owners and moderators post (up to
// 500 characters, with the chat's emoji panel and ":" suggestions), the owner deletes any
// announcement and an author their own (asked first), and anyone who sees the list likes and
// unlikes. Pages of 10 load as the list's end comes into view; failures are toasted by the session.

import { useRef, useState } from 'react'
import { Avatar, Button, DclLogo, MaskIcon, Spinner, TextArea, Trash, VerifiedBadge, showConfirm } from '../../design'
import heartOutline from '../../assets/communities/favourite-outline.png'
import heartSolid from '../../assets/communities/favourite-solid.png'
import { splitName } from '@explorer-core/lib/identity'
import type { CommunityPost } from '@explorer-core/engine/protocol'
import { EmojiPicker } from '../chat/EmojiPicker'
import { EmojiSuggestList, emojiSuggestionAt, moveSelection, type EmojiSuggestion } from '../chat/EmojiSuggestList'
import { isCancelKey } from '../../lib/bindingLabels'
import { knownUserColor } from '../session/profileStore'
import type { CommunityPosts } from '../session/useEngineSession'
import { LoadMore } from './LoadMore'
import styles from './Announcements.module.css'

export const ANNOUNCEMENT_MAX = 500
export const NO_ANNOUNCEMENTS = 'No Announcements here yet!'
const PLACEHOLDER = 'Any Announcement to share with your Community?'

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']

/** An announcement's age: "Now", "5m", "3h", then "Mar 4", with the
 *  year when it is not this one (UTC). */
export function postAge(ts: number, now = Date.now()): string {
  const minutes = (now - ts) / 60_000
  if (minutes < 1) return 'Now'
  if (minutes < 60) return `${Math.floor(minutes)}m`
  if (minutes < 24 * 60) return `${Math.floor(minutes / 60)}h`
  const d = new Date(ts)
  const day = `${MONTHS[d.getUTCMonth()]} ${d.getUTCDate()}`
  return d.getUTCFullYear() === new Date(now).getUTCFullYear() ? day : `${day}, ${d.getUTCFullYear()}`
}

function AnnouncementCard({
  post,
  canDelete,
  liking,
  deleting,
  onLike,
  onDelete
}: {
  post: CommunityPost
  canDelete: boolean
  liking: boolean
  /** Its delete is on its way: the button is held. */
  deleting: boolean
  onLike: (like: boolean) => void
  onDelete: () => void
}): React.JSX.Element {
  const name = splitName(post.author).base
  // the like's mark grows and fades once
  const [burst, setBurst] = useState(0)
  const askDelete = (): void => {
    void showConfirm({ title: 'Are you sure you want to delete this Announcement?', confirmLabel: 'DELETE', cancelLabel: 'CANCEL' }).then((yes) => {
      if (yes) onDelete()
    })
  }
  return (
    <article className={styles.card} aria-label={`Announcement by ${name}`}>
      <Avatar src={post.authorPicture} name={name} color={knownUserColor(post.authorAddress, post.author)} size={36} />
      <div className={styles.body}>
        <div className={styles.head}>
          <span className={styles.author}>{name}</span>
          {post.hasClaimedName ? <VerifiedBadge size={14} /> : <span className={styles.tag}>#{post.authorAddress.slice(-4)}</span>}
          {post.official && (
            <span className={styles.official} title="Official" aria-label="Official">
              <DclLogo size={14} />
            </span>
          )}
          <span className={styles.age}>· {postAge(post.timestamp)}</span>
        </div>
        <p className={styles.text}>{post.text}</p>
        <div className={styles.foot}>
          <button
            type="button"
            className={`${styles.like} ${post.likedByMe ? styles.liked : ''}`.trim()}
            aria-label={post.likedByMe ? 'Unlike' : 'Like'}
            aria-pressed={post.likedByMe}
            disabled={liking}
            onClick={() => {
              if (!post.likedByMe) setBurst((n) => n + 1)
              onLike(!post.likedByMe)
            }}
          >
            <span className={styles.heart}>
              <MaskIcon src={post.likedByMe ? heartSolid : heartOutline} size={16} />
              {burst > 0 && (
                <span key={burst} className={styles.burst}>
                  <MaskIcon src={heartSolid} size={16} />
                </span>
              )}
            </span>
            {post.likes}
          </button>
          {canDelete && (
            <button type="button" className={styles.delete} aria-label="Delete announcement" disabled={deleting} onClick={askDelete}>
              <Trash size={16} />
            </button>
          )}
        </div>
      </div>
    </article>
  )
}

function Composer({
  me,
  posting,
  posted,
  onPost
}: {
  me: { address: string; name: string; picture?: string } | null
  posting: boolean
  /** Counts the successful posts: the text clears after each. */
  posted: number
  onPost: (content: string) => void
}): React.JSX.Element {
  const [draft, setDraft] = useState('')
  const [picker, setPicker] = useState(false)
  const [sug, setSug] = useState<EmojiSuggestion | null>(null)
  const field = useRef<HTMLTextAreaElement | null>(null)
  // the text clears once a post went through, not when it is sent
  const [seen, setSeen] = useState(posted)
  if (seen !== posted) {
    setSeen(posted)
    setDraft('')
    setSug(null)
  }
  // The limit counts UTF-16 units, as the field's own maxLength does: most emoji take two. An
  // emoji that doesn't fit is dropped rather than cut in half.
  const replaceRange = (start: number, end: number, text: string): void => {
    const next = draft.slice(0, start) + text + draft.slice(end)
    if (next.length > ANNOUNCEMENT_MAX) return
    const caret = Math.min(start + text.length, next.length)
    setDraft(next)
    setSug(null)
    requestAnimationFrame(() => {
      const el = field.current
      if (el == null) return
      el.focus()
      el.setSelectionRange(caret, caret)
    })
  }
  const insertAtCaret = (glyph: string): void => {
    const el = field.current
    const start = el?.selectionStart ?? draft.length
    const end = el?.selectionEnd ?? draft.length
    replaceRange(start, end, glyph)
  }
  const accept = (s: EmojiSuggestion, i: number): void => replaceRange(s.start, s.end, s.items[i].emoji)
  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>): void => {
    e.stopPropagation() // keep movement keys out of the engine while typing
    if (e.nativeEvent.isComposing) return
    if (sug != null && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
      e.preventDefault()
      setSug(moveSelection(sug, e.key === 'ArrowDown' ? 1 : -1))
    } else if (sug != null && (e.key === 'Enter' || e.key === 'Tab')) {
      e.preventDefault()
      accept(sug, sug.sel)
    } else if (isCancelKey(e)) {
      // as the chat's input: Cancel closes the suggestions and the emoji panel and leaves the field,
      // so the next Cancel closes the card
      e.preventDefault()
      setSug(null)
      setPicker(false)
      e.currentTarget.blur()
    }
  }
  const myName = me != null ? splitName(me.name).base : ''
  return (
    <div className={styles.composer}>
      <Avatar src={me?.picture} name={myName} color={me != null ? knownUserColor(me.address, me.name) : undefined} size={36} />
      <div className={styles.compose}>
        <TextArea
          className={styles.field}
          rows={3}
          value={draft}
          maxLength={ANNOUNCEMENT_MAX}
          placeholder={PLACEHOLDER}
          aria-label="New announcement"
          onChange={setDraft}
          onKeyDown={onKeyDown}
          onSelect={(e) => {
            const el = e.currentTarget
            field.current = el
            if (el.selectionStart === el.selectionEnd && sug?.end !== el.selectionStart) setSug(emojiSuggestionAt(el.value, el.selectionStart ?? el.value.length))
          }}
          onBlur={() => setSug(null)}
        />
        {sug != null && <EmojiSuggestList className={styles.suggest} suggestion={sug} onAccept={(i) => accept(sug, i)} />}
        {picker && (
          <div className={styles.picker}>
            <EmojiPicker
              onPick={insertAtCaret}
              onClose={() => {
                setPicker(false)
                field.current?.focus()
              }}
            />
          </div>
        )}
        <div className={styles.composeFoot}>
          <button
            type="button"
            className={`${styles.emojiBtn} ${picker ? styles.emojiOn : ''}`.trim()}
            aria-label="Emoji"
            aria-pressed={picker}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => setPicker((p) => !p)}
          />
          {draft.length > 0 && (
            <span className={styles.count}>
              {draft.length}/{ANNOUNCEMENT_MAX}
            </span>
          )}
          <Button size="sm" disabled={draft.trim() === '' || posting} onClick={() => onPost(draft.trim())} aria-label="Post announcement">
            {posting ? <Spinner size={14} /> : 'POST'}
          </Button>
        </div>
      </div>
    </div>
  )
}

export function Announcements({
  posts,
  role,
  me,
  onLoadMore,
  onPost,
  onDelete,
  onLike,
  locked = false
}: {
  posts: CommunityPosts | null
  /** The user's role in the community: owners and moderators post, the owner deletes any. */
  role: string
  me: { address: string; name: string; picture?: string } | null
  onLoadMore: () => void
  onPost: (content: string) => void
  onDelete: (postId: string) => void
  onLike: (postId: string, like: boolean) => void
  /** A private community the user isn't a member of: its announcements aren't theirs to read. */
  locked?: boolean
}): React.JSX.Element | null {
  if (locked) return null
  if (posts == null || posts.loading) return <div className={styles.empty}>Loading…</div>
  const canPost = role === 'owner' || role === 'moderator'
  const mine = (address: string): boolean => me != null && me.address.toLowerCase() === address.toLowerCase()
  return (
    <div className={styles.list}>
      {canPost && <Composer me={me} posting={posts.posting} posted={posts.posted} onPost={onPost} />}
      {canPost && posts.total > 0 && <hr className={styles.separator} />}
      {/* a first page that could not be read is toasted, not shown as an empty list */}
      {posts.posts.length === 0 && !canPost && posts.failed !== true && <div className={styles.empty}>{NO_ANNOUNCEMENTS}</div>}
      {posts.posts.map((p) => (
        <AnnouncementCard
          key={p.id}
          post={p}
          canDelete={role === 'owner' || mine(p.authorAddress)}
          liking={posts.liking.includes(p.id)}
          deleting={posts.deleting.includes(p.id)}
          onLike={(like) => onLike(p.id, like)}
          onDelete={() => onDelete(p.id)}
        />
      ))}
      {posts.posts.length < posts.total && <LoadMore count={posts.posts.length} onVisible={onLoadMore} />}
      {posts.loadingMore && (
        <div className={styles.loadingMore}>
          <Spinner size={20} />
        </div>
      )}
    </div>
  )
}
