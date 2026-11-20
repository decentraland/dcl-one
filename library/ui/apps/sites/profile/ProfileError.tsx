import { Button } from '../../../index'

export function ProfileError({
  title,
  body,
  onRetry
}: {
  title: string
  body: string
  onRetry?: () => void
}) {
  return (
    <main role="alert">
      <h1>{title}</h1>
      <p>{body}</p>
      {onRetry ? (
        <Button variant="secondary" size="md" onClick={onRetry}>
          Try again
        </Button>
      ) : null}
    </main>
  )
}
