import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { AllEvents } from '../../apps/sites/events/AllEvents'
import { LiveNow } from '../../apps/sites/events/LiveNow'
import { Upcoming } from '../../apps/sites/events/Upcoming'
import { DayCard, UpcomingCard } from '../../apps/sites/events/cards'
import '../../foundations/foundations.css'
import cover from '../../apps/sites/assets/events/top_background.webp'

const base = {
  id: 'event',
  title: 'A long event title that wraps over two full lines on every supported card width',
  image: null,
  href: '/events?id=event',
  creator: { name: 'Creator', href: '/profile/creator' },
  time: { label: 'Tomorrow at noon', dateTime: '2026-11-13T12:00:00Z' },
  remind: { active: false, pending: false, onToggle: () => {} },
  calendarHref: 'https://calendar.google.com/calendar/render?action=TEMPLATE'
}
function TestDayCard() {
  const [active, setActive] = useState(false)
  return (
    <DayCard
      {...base}
      live={false}
      remind={{ active, pending: false, onToggle: () => setActive(!active) }}
    />
  )
}

const day = {
  key: 'today',
  label: 'Today',
  fullLabel: 'Today',
  isToday: true,
  cards: [
    {
      ...base,
      title: 'Live in the day column',
      image: cover,
      live: true,
      users: 0,
      jumpIn: { href: '/play?realm=gallery.dcl.eth' }
    },
    ...Array.from({ length: 7 }, (_, index) => ({
      ...base,
      id: `future-${index}`,
      title: `Future event ${index + 1}`,
      image: cover,
      live: false
    }))
  ]
}

createRoot(document.getElementById('root')!).render(
  <main>
    <LiveNow
      cards={[
        { ...base, live: true, jumpIn: { href: '/play' } },
        { ...base, id: 'other', live: false, jumpIn: { href: '/play' } }
      ]}
    />
    <Upcoming
      cards={[
        base,
        {
          ...base,
          id: 'another',
          creator: undefined,
          title:
            'A deliberately very long event title which must be clamped after two complete lines without cutting the second line'
        }
      ]}
    />
    <section data-day>
      <TestDayCard />
    </section>
    <section data-live-day>
      <DayCard {...base} live users={0} jumpIn={{ href: '/play?realm=gallery.dcl.eth' }} />
    </section>
    <section data-live-small>
      <UpcomingCard
        {...base}
        id="live-small"
        title="Live party"
        remind={undefined}
        live
        users={16}
        usersLabel="16 online"
        jumpIn={{ href: '/play' }}
      />
    </section>
    <div data-calendar>
      <AllEvents
        days={[day]}
        layout={window.innerWidth < 768 ? 'list' : 'columns'}
        canGoBack={false}
        onPrevious={() => {}}
        onNext={() => {}}
      />
    </div>
  </main>
)
