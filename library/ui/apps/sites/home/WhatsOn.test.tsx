import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { WhatsOn } from './WhatsOn'

const page = (image: string) =>
  renderToStaticMarkup(
    <WhatsOn
      title="What's On"
      cards={[{ id: 'ev', title: 'Party', image, href: '/whats-on?id=ev' }]}
      labels={{ live: 'Live', by: 'by', jumpIn: 'Jump In' }}
      carouselLabels={{
        previous: 'Previous',
        next: 'Next',
        goTo: (n) => `Go to ${n}`,
        position: (n, total) => `${n} of ${total}`
      }}
      viewAll={{ label: 'View All', href: '/whats-on' }}
    />
  )

describe('WhatsOn', () => {
  it('shows the image of a card as its background', () => {
    expect(page('/content/contents/bafy1')).toContain(
      'background-image:url(&quot;/content/contents/bafy1&quot;)'
    )
  })

  it('escapes an image address that has a quote, so it adds no CSS of its own', () => {
    const html = page('https://example.org/a.png"); color: red; x: url("')
    expect(html).toContain(
      'background-image:url(&quot;https://example.org/a.png\\&quot;); color: red; x: url(\\&quot;&quot;)'
    )
    expect(html).not.toContain('a.png&quot;)')
  })
})

describe('when the live event has a start and public attendees', () => {
  it('should expose the start, the real profile picture and the copy-link action', () => {
    const html = renderToStaticMarkup(
      <WhatsOn
        title="What's On"
        cards={[
          {
            id: 'ev',
            title: 'Party',
            image: null,
            href: '/events?id=ev',
            users: 1,
            usersLabel: '1 online',
            share: true,
            started: { dateTime: '2026-11-27T10:00:00.000Z', minutes: 5 },
            attendees: [{ id: 'ada', name: 'Ada', avatarUrl: '/profile-images/entities/face.png' }]
          }
        ]}
        labels={{
          live: 'Live',
          by: 'by',
          jumpIn: 'Jump In',
          activity: {
            started: (minutes) => `Started ${minutes} min ago`,
            attendees: 'Event attendees',
            copyLink: (title) => `Copy event link: ${title}`,
            copied: 'Copied',
            copyFailed: 'Could not copy'
          }
        }}
        carouselLabels={{
          previous: 'Previous',
          next: 'Next',
          goTo: (n) => `Go to ${n}`,
          position: (n, total) => `${n} of ${total}`
        }}
        viewAll={{ label: 'View All', href: '/events' }}
      />
    )

    expect(html).toContain('dateTime="2026-11-27T10:00:00.000Z">Started 5 min ago')
    expect(html).toContain('src="/profile-images/entities/face.png"')
    expect(html).toContain('aria-label="Copy event link: Party"')
  })
})
