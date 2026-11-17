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
