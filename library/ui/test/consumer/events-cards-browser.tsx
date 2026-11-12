import { createRoot } from 'react-dom/client'
import { LiveNow } from '../../apps/sites/events/LiveNow'
import { Upcoming } from '../../apps/sites/events/Upcoming'
import { ExperienceCardSlot } from '../../apps/sites/events/cards/ExperienceCardSlot'
import '../../foundations/foundations.css'
const base = {
  id: 'event',
  title: 'A long event title that wraps',
  image: null,
  href: '/events?id=event',
  creator: { name: 'Creator', href: '/profile/creator' },
  time: { label: 'Tomorrow at noon', dateTime: '2026-11-13T12:00:00Z' },
  remind: { active: false, pending: false, onToggle: () => {} },
  calendarHref: 'https://calendar.google.com/calendar/render?action=TEMPLATE'
}
createRoot(document.getElementById('root')!).render(
  <main>
    <LiveNow
      cards={[
        { ...base, live: true, jumpIn: { href: '/play' } },
        { ...base, id: 'other', live: false, jumpIn: { href: '/play' } }
      ]}
    />
    <Upcoming cards={[base, { ...base, id: 'another' }]} />
    <section data-day>
      <ExperienceCardSlot {...base} live={false} />
    </section>
  </main>
)
