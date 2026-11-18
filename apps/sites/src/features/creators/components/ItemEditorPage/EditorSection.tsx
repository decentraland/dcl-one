import { useState, type ReactNode } from 'react'
import { ExpandMoreIcon as ChevronIcon } from '@/features/creators/components/Icons'
import { Tooltip } from '@/features/creators/components/Tooltip'
import styles from './ItemEditorPage.module.css'

type Props = {
  title: string
  children: ReactNode
  defaultOpen?: boolean
  /** Glyph drawn before the title. */
  icon?: ReactNode
  /** Header shows only the glyph (collapsed sidebar); the title stays as its accessible name. */
  iconOnly?: boolean
  /** Drawn after the title so it stays visible while the section is collapsed. */
  adornment?: ReactNode
  testId: string
}

/** A collapsible surface with an uppercase title, the inspector's container format. */
export function EditorSection({
  title,
  children,
  defaultOpen = true,
  icon,
  iconOnly = false,
  adornment,
  testId
}: Props) {
  const [isOpen, setOpen] = useState(defaultOpen)
  const header = (
    <button
      className={styles.sectionHeader}
      type="button"
      aria-expanded={isOpen}
      aria-label={iconOnly ? title : undefined}
      data-icon-only={iconOnly || undefined}
      data-testid={`${testId}-toggle`}
      onClick={() => setOpen(open => !open)}
    >
      {icon && (
        <span className={styles.sectionIcon} aria-hidden>
          {icon}
        </span>
      )}
      {!iconOnly && <span className={styles.sectionTitle}>{title}</span>}
      {!iconOnly && adornment}
      {!iconOnly && <ChevronIcon data-chevron fontSize="small" />}
    </button>
  )
  return (
    <section className={styles.section} data-testid={testId} data-open={isOpen || undefined}>
      {iconOnly ? (
        <Tooltip content={title} placement="right" asChild testId={`${testId}-tooltip`}>
          {header}
        </Tooltip>
      ) : (
        header
      )}
      {isOpen && (
        <div className={styles.sectionBody} data-testid={`${testId}-body`}>
          {children}
        </div>
      )}
    </section>
  )
}
