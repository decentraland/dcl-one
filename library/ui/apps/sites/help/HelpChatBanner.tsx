import { useId } from 'react'
import helpGirl from '../assets/help/help_girl.webp'
import { AnimatedBackground } from '../shared/AnimatedBackground/AnimatedBackground'
import { Button } from '../shared/Button/Button'
import styles from './HelpChatBanner.module.css'

type HelpChatBannerProps = {
  title: string
  subtitle: string
  actionLabel: string
} & ({ onAction: () => void; href?: never } | { href: string; onAction?: never })

function HelpChatBanner({ title, subtitle, actionLabel, onAction, href }: HelpChatBannerProps) {
  const titleId = useId()
  return (
    <aside className={styles.root} aria-labelledby={titleId}>
      <div className={styles.backdrop}>
        <AnimatedBackground />
      </div>
      <div className={styles.texts}>
        <h2 id={titleId} className={styles.title}>
          {title}
        </h2>
        <p className={styles.subtitle}>{subtitle}</p>
        <Button
          className={styles.action}
          {...(href !== undefined ? { href } : { onClick: onAction })}
        >
          {actionLabel}
        </Button>
      </div>
      <img className={styles.image} src={helpGirl} alt="" aria-hidden="true" />
    </aside>
  )
}

export { HelpChatBanner }
export type { HelpChatBannerProps }
