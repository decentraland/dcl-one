import { createRoot } from 'react-dom/client'
import { BlogCard } from '@dcl/one-ui'
import '../../foundations/foundations.css'

// A loaded card and its skeleton side by side, each in its own 300px column, so neither takes the other's height:
// the loaded card has the content a Sites card has (a date, a two-line title, a 24px badge, an author).
const badge = (
  <span style={{ display: 'inline-flex', alignItems: 'center', height: 24 }}>Guides</span>
)
const column = {
  width: 300,
  display: 'flex',
  flexDirection: 'column' as const,
  alignItems: 'stretch'
}
const cover = (
  <img
    alt=""
    src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="
  />
)
const title = 'Builders meet at Genesis Plaza for the new season'

createRoot(document.getElementById('root')!).render(
  <main
    style={{
      display: 'grid',
      gridTemplateColumns: 'repeat(4, 300px)',
      gap: 16,
      alignItems: 'start'
    }}
  >
    <div id="standard-loaded" style={column}>
      <BlogCard
        title={title}
        href="#post"
        cover={cover}
        metadata="Jan 2, 2026"
        category={badge}
        author="Ada"
      />
    </div>
    <div id="standard-loading" style={column}>
      <BlogCard loading />
    </div>
    <div id="related-loaded" style={column}>
      <BlogCard
        variant="related"
        title={title}
        href="#post"
        cover={cover}
        metadata="Jan 2, 2026"
        category={badge}
      />
    </div>
    <div id="related-loading" style={column}>
      <BlogCard variant="related" loading loadingLabel="Loading a related post" />
    </div>
  </main>
)
