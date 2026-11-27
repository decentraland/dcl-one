import { Button } from '@dcl/one-ui'
import { useId, type ReactNode } from 'react'
import helpGirl from '../assets/help/help_girl.webp'
import { AnimatedBackground } from '../../../components/AnimatedBackground/AnimatedBackground'
import styles from './HelpChatBanner.module.css'

type HelpChatBannerProps = {
  title: string
  subtitle: ReactNode
  actionLabel: string
} & (
  | { onAction: () => void; emailAction?: never }
  | { onAction?: () => void; emailAction: { address: string; label: string } }
)

function focusEmail(link: HTMLAnchorElement | null) {
  link?.focus()
}

/** The banner that stays at the bottom of the help center and opens the support chat. */
function HelpChatBanner({
  title,
  subtitle,
  actionLabel,
  onAction,
  emailAction
}: HelpChatBannerProps) {
  const titleId = useId()
  return (
    <aside className={styles.root} aria-labelledby={titleId}>
      {!emailAction && (
        <div className={styles.backdrop}>
          <AnimatedBackground />
        </div>
      )}
      <div className={styles.texts}>
        <h2 id={titleId} className={styles.title}>
          {title}
        </h2>
        <p className={styles.subtitle} aria-live="polite">
          {subtitle}
        </p>
        {emailAction && (
          <Button
            ref={focusEmail}
            size="lg"
            variant="secondary"
            className={styles.action}
            href={`mailto:${emailAction.address}`}
          >
            {emailAction.label}
          </Button>
        )}
        {onAction && (
          <Button size="lg" className={styles.action} onClick={onAction}>
            {actionLabel}
          </Button>
        )}
      </div>
      {!emailAction && <img className={styles.image} src={helpGirl} alt="" aria-hidden="true" />}
    </aside>
  )
}

export { HelpChatBanner }
export type { HelpChatBannerProps }
