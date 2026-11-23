import { createRoot } from 'react-dom/client'
import { BlogCard } from '@dcl/one-ui'
import '../../foundations/foundations.css'

// A loaded card and its skeleton side by side, each in its own column, so neither takes the other's height. The
// columns are as wide as the gallery's Blog grids make them at the viewport under test (`?standard=…&related=…`), and
// the loaded card carries what a loading card reserves: the gallery's two-line title and a 24px category badge.
const query = new URLSearchParams(location.search)
const width = (name: string) => Number(query.get(name) ?? 300)
const badge = (
  <span style={{ display: 'inline-flex', alignItems: 'center', height: 24 }}>Guides</span>
)
const column = (px: number) => ({ width: px, display: 'flex', flexDirection: 'column' as const })
const cover = (
  <img
    alt=""
    src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="
  />
)
const title = 'Creator Fund: rewards for indie game creators'

createRoot(document.getElementById('root')!).render(
  <main style={{ display: 'flex', flexWrap: 'wrap', gap: 16, alignItems: 'flex-start' }}>
    <div id="standard-loaded" style={column(width('standard'))}>
      <BlogCard
        title={title}
        href="#post"
        cover={cover}
        metadata="Jan 2, 2026"
        category={badge}
        author="Ada"
      />
    </div>
    <div id="standard-loading" style={column(width('standard'))}>
      <BlogCard loading />
    </div>
    <div id="related-loaded" style={column(width('related'))}>
      <BlogCard
        variant="related"
        title={title}
        href="#post"
        cover={cover}
        metadata="Jan 2, 2026"
        category={badge}
      />
    </div>
    <div id="related-loading" style={column(width('related'))}>
      <BlogCard variant="related" loading loadingLabel="Loading a related post" />
    </div>
  </main>
)
