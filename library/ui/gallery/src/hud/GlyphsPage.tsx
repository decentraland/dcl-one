import type { ComponentType, ReactNode } from 'react'
import * as Glyphs from '../../../apps/hud/design/Glyphs'
import type { MarkName, SocialIconName } from '../../../apps/hud/design/Glyphs'
import styles from './GlyphsPage.module.css'

// The names SocialIcon and Mark draw, every one: the type check fails when one is added.
const SOCIAL_NAMES = Object.keys({
  discord: 1,
  reddit: 1,
  github: 1,
  twitter: 1
} satisfies Record<SocialIconName, 1>) as SocialIconName[]
const MARK_NAMES = Object.keys({
  close: 1,
  check: 1,
  reset: 1,
  dot: 1,
  kebab: 1,
  down: 1,
  up: 1,
  right: 1
} satisfies Record<MarkName, 1>) as MarkName[]

function Tile({ name, children }: { name: string; children: ReactNode }) {
  return (
    <div className={styles.tile}>
      <span className={styles.glyph}>{children}</span>
      <span className={styles.name}>{name}</span>
    </div>
  )
}

/** Every glyph the HUD's Glyphs.tsx exports, at its default size; one drawn from a name shows each name. */
export function GlyphGrid() {
  return (
    <div className={styles.grid}>
      {Object.entries(Glyphs).flatMap(([name, value]) => {
        if (name === 'SocialIcon')
          return SOCIAL_NAMES.map((social) => (
            <Tile key={`${name} ${social}`} name={`${name} ${social}`}>
              <Glyphs.SocialIcon name={social} />
            </Tile>
          ))
        if (name === 'Mark')
          return MARK_NAMES.map((mark) => (
            <Tile key={`${name} ${mark}`} name={`${name} ${mark}`}>
              <Glyphs.Mark name={mark} size={1.5} />
            </Tile>
          ))
        const Glyph = value as ComponentType
        return [
          <Tile key={name} name={name}>
            <Glyph />
          </Tile>
        ]
      })}
    </div>
  )
}
