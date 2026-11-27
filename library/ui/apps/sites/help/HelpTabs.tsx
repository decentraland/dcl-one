import { Tabs } from '../../../components/Tabs/Tabs'
import { TabsBar } from '../../../components/TabsBar/TabsBar'
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
 * The help center's section switcher: the shared pill tabs, which wrap onto rows. The arrow keys, Home and End
 * move focus and select (TabsBar).
 */
function HelpTabs<Id extends string>({
  tabs,
  active,
  onChange,
  label,
  tabId,
  panelId
}: HelpTabsProps<Id>) {
  return (
    <TabsBar label={label} variant="pill">
      {tabs.map((tab) => {
        const Icon = ICONS[tab.icon]
        return (
          <Tabs
            key={tab.id}
            id={tabId(tab.id)}
            variant="pill"
            selected={tab.id === active}
            aria-controls={panelId}
            onClick={() => onChange(tab.id)}
            label={
              <span className={styles.label}>
                <Icon size={20} aria-hidden="true" focusable="false" />
                {tab.label}
              </span>
            }
          />
        )
      })}
    </TabsBar>
  )
}

export { HelpTabs }
export type { HelpTab, HelpTabIcon, HelpTabsProps }
