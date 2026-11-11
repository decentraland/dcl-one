import { useId, useState, type ReactNode } from 'react'
import { GeneralDownArrow } from '../../../foundations/icons'
import styles from './HelpAccordion.module.css'

type HelpAccordionItem = { id: string; question: string; answer: ReactNode }

type HelpAccordionProps = {
  items: readonly HelpAccordionItem[]
  /** Called when a question opens, not when it closes. */
  onOpen?: (id: string) => void
}

/**
 * Questions that open one at a time: opening one closes the one before. Each question is a button that controls
 * its answer region. TODO(figma-ds): replace with the design system's Accordion once it exists.
 */
function HelpAccordion({ items, onOpen }: HelpAccordionProps) {
  const prefix = useId()
  const [open, setOpen] = useState<string | null>(null)

  const toggle = (id: string) => {
    if (open === id) {
      setOpen(null)
      return
    }
    setOpen(id)
    onOpen?.(id)
  }

  return (
    <div className={styles.list}>
      {items.map((item, index) => {
        const expanded = open === item.id
        const buttonId = `${prefix}-q${index}`
        const panelId = `${prefix}-a${index}`
        return (
          <div
            key={item.id}
            className={[styles.item, expanded && styles.expanded].filter(Boolean).join(' ')}
          >
            <h3 className={styles.heading}>
              <button
                type="button"
                id={buttonId}
                className={styles.trigger}
                aria-expanded={expanded}
                aria-controls={panelId}
                onClick={() => toggle(item.id)}
              >
                <span className={styles.question}>{item.question}</span>
                <span className={styles.icon} aria-hidden="true">
                  <GeneralDownArrow size={24} focusable="false" />
                </span>
              </button>
            </h3>
            <div
              id={panelId}
              role="region"
              aria-labelledby={buttonId}
              className={styles.answer}
              hidden={!expanded}
            >
              {item.answer}
            </div>
          </div>
        )
      })}
    </div>
  )
}

export { HelpAccordion }
export type { HelpAccordionItem, HelpAccordionProps }
