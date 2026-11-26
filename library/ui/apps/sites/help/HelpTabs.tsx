import { useRef, type KeyboardEvent } from 'react'
import { GeneralHelp, GeneralSupport, ShopCredits } from '../../../foundations/icons'
import styles from './HelpTabs.module.css'

type HelpTabIcon = 'faq' | 'creatorFund' | 'supportUpdates'

type HelpTab<Id extends string = string> = { id: Id; label: string; icon: HelpTabIcon }

type HelpTabsProps<Id extends string = string> = {
  tabs: readonly HelpTab<Id>[]
  active: Id
  onChange: (id: Id) => void
  /** Names the tab list for assistive technology. */
  label: string
  tabId: (id: Id) => string
  panelId: string
}

const ICONS = {
  faq: GeneralHelp,
  creatorFund: ShopCredits,
  supportUpdates: GeneralSupport
} as const

/**
 * The help center's section switcher: a tab list with automatic activation. The arrow keys of either axis (the
 * list is a column on wide screens and wrapping rows on phones), Home and End move focus and select.
 * TODO(figma-ds): replace with the design system's Tabs once it exists.
 */
function HelpTabs<Id extends string>({
  tabs,
  active,
  onChange,
  label,
  tabId,
  panelId
}: HelpTabsProps<Id>) {
  const refs = useRef(new Map<Id, HTMLButtonElement>())

  const select = (index: number) => {
    const tab = tabs[(index + tabs.length) % tabs.length]
    if (!tab) return
    onChange(tab.id)
    refs.current.get(tab.id)?.focus()
  }

  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    switch (event.key) {
      case 'ArrowRight':
      case 'ArrowDown':
        select(index + 1)
        break
      case 'ArrowLeft':
      case 'ArrowUp':
        select(index - 1)
        break
      case 'Home':
        select(0)
        break
      case 'End':
        select(tabs.length - 1)
        break
      default:
        return
    }
    event.preventDefault()
  }

  return (
    <div className={styles.root} role="tablist" aria-label={label}>
      {tabs.map((tab, index) => {
        const selected = tab.id === active
        const Icon = ICONS[tab.icon]
        return (
          <button
            key={tab.id}
            ref={(node) => {
              if (node) refs.current.set(tab.id, node)
              else refs.current.delete(tab.id)
            }}
            type="button"
            role="tab"
            id={tabId(tab.id)}
            aria-selected={selected}
            aria-controls={panelId}
            tabIndex={selected ? 0 : -1}
            className={[styles.tab, selected && styles.selected].filter(Boolean).join(' ')}
            onClick={() => onChange(tab.id)}
            onKeyDown={(event) => onKeyDown(event, index)}
          >
            <Icon size={20} aria-hidden="true" focusable="false" />
            {tab.label}
          </button>
        )
      })}
    </div>
  )
}

export { HelpTabs }
export type { HelpTab, HelpTabIcon, HelpTabsProps }
