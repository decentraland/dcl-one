import { Button } from '../../../index'
import { GeneralWarning } from '../../../foundations/icons'

import styles from './CommunityError.module.css'

const COPY = {
  'not-found': {
    title: 'Community not found',
    body: 'The community you are looking for does not exist.'
  },
  unavailable: {
    title: 'Community unavailable',
    body: "We couldn't load this community right now. Try again in a moment."
  }
} as const

export type CommunityErrorReason = keyof typeof COPY

/** An unknown community, or one that could not be read; only the second offers Try again. */
export function CommunityError({
  reason,
  onRetry
}: {
  reason: CommunityErrorReason
  onRetry: () => void
}) {
  const copy = COPY[reason]
  return (
    <main className={styles.root} role="alert">
      <GeneralWarning className={styles.icon} size={72} aria-hidden="true" />
      <h1 className={styles.title}>{copy.title}</h1>
      <p className={styles.body}>{copy.body}</p>
      {reason === 'unavailable' ? (
        <Button variant="secondary" size="md" onClick={onRetry}>
          Try again
        </Button>
      ) : null}
    </main>
  )
}
