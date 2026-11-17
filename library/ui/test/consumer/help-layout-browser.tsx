import { createRoot } from 'react-dom/client'
import { HelpPage } from '../../apps/sites/help/HelpPage'
import { HelpSection } from '../../apps/sites/help/HelpSection'
import '../../foundations/foundations.css'

const tabs = [
  { id: 'faq', label: 'FAQ', icon: 'faq' },
  { id: 'creator-fund', label: 'Creator Fund', icon: 'creatorFund' },
  { id: 'support-updates', label: 'Support Updates', icon: 'supportUpdates' }
] as const

createRoot(document.getElementById('root')!).render(
  <HelpPage
    title="Help Center"
    description="Browse through FAQs and Support Updates or speak with the Support Team directly using the chat box on the bottom right."
    tabs={tabs}
    activeTab="faq"
    onTabChange={() => {}}
    status={
      <span data-status style={{ display: 'block', width: 140, height: 37 }}>
        System Status
      </span>
    }
  >
    <HelpSection title="Frequently Asked Questions">
      <div data-questions style={{ height: 300 }} />
    </HelpSection>
  </HelpPage>
)
