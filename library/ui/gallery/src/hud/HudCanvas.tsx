import { SHOWCASE_SECTIONS } from '../../../apps/hud/design/Showcase'
import frame from '../frame/Canvas.module.css'
import { ExampleView } from '../frame/ExampleView'
import { HUD_GLYPHS, HUD_OVERVIEW } from '../hudPages'
import { GlyphGrid } from './GlyphsPage'
import styles from './HudCanvas.module.css'

/** The Showcase's sections, then the glyphs the gallery draws itself. */
export const hudCanvasPages = [...SHOWCASE_SECTIONS, { ...HUD_GLYPHS, render: () => <GlyphGrid /> }]

/** One page of the HUD section, or every page on the overview, each on a stage of its own. */
export function HudCanvas({ search }: { search: string }) {
  const name = new URLSearchParams(search).get('hud')
  const pages =
    name === HUD_OVERVIEW ? hudCanvasPages : hudCanvasPages.filter((page) => page.name === name)
  if (!pages.length)
    return (
      <p role="alert" className={frame.error}>
        No HUD section matches {search || 'an empty query'}.
      </p>
    )
  return (
    <main className={`${frame.canvas} ${styles.window}`} data-hud={name}>
      {pages.map((page) => (
        <ExampleView
          key={page.name}
          example={{ id: page.name, title: page.title, render: page.render }}
        />
      ))}
    </main>
  )
}
