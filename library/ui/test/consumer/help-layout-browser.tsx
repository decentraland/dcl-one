import { createRoot } from 'react-dom/client'
import { HelpChatBanner } from '../../apps/sites/help/HelpChatBanner'
import { HelpPage } from '../../apps/sites/help/HelpPage'
import { HelpSection } from '../../apps/sites/help/HelpSection'
import { HelpStatus } from '../../apps/sites/help/HelpStatus'
import '../../foundations/foundations.css'

const tabs = [
  { id: 'faq', label: 'FAQ', icon: 'faq' },
  { id: 'creator-fund', label: 'Creator Fund', icon: 'creatorFund' },
  { id: 'support-updates', label: 'Support Updates', icon: 'supportUpdates' }
] as const

// The tallest section the page renders (Creator Fund): a description, a list longer than the space and a footer.
createRoot(document.getElementById('root')!).render(
  <HelpPage
    title="Help Center"
    description="Browse through FAQs and Support Updates or speak with the Support Team directly using the chat box on the bottom right."
    tabs={tabs}
    activeTab="creator-fund"
    onTabChange={() => {}}
    status={
      <HelpStatus
        label="System Status"
        report={{
          overall: 'ok',
          overallText: 'All systems operational',
          services: [
            { name: 'Catalyst', state: 'ok' },
            { name: 'Marketplace', state: 'ok' }
          ]
        }}
        detailsLabel="See details"
        detailsHref="https://status.example.com"
      />
    }
    banner={
      <HelpChatBanner
        title="Need more help?"
        subtitle="Our Support Team is here to help."
        actionLabel="Chat now"
        onAction={() => {}}
      />
    }
  >
    <HelpSection
      title="Creator Fund"
      description="Answers about the Creator Fund program and its payouts."
      footer="Write to the Creator Fund team for anything else."
    >
      <div data-questions style={{ height: 1200 }} />
    </HelpSection>
  </HelpPage>
)
