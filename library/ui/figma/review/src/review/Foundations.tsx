import type { ComponentType } from 'react'
import data from './catalog.json'
import * as generatedIcons from '../generated/icons'
import { breakpoints, gradients } from '../generated/tokens/tokens'
import { theme } from '../generated/tokens/theme'
import textStyles from '../generated/tokens/text-styles.module.css'
import styles from './Foundations.module.css'

const sections = [
  'tokens',
  'colors',
  'gradients',
  'fonts',
  'typography',
  'spacing',
  'borders',
  'layout',
  'icons',
  'effects'
] as const
type Section = (typeof sections)[number]

// Icons render through the generated icon components, never through Figma's own SVG exports. An icon with an
// active state takes `active`, so each variant tile draws its own art; no other icon is passed the prop.
const iconComponents = new Map(
  generatedIcons.iconMeta.map((meta) => {
    const Icon: ComponentType<generatedIcons.ActiveIconProps> =
      generatedIcons.isActiveIconComponentName(meta.component)
        ? generatedIcons.activeIconComponents[meta.component]
        : generatedIcons.iconComponents[meta.component]
    return [meta.name, { hasActiveState: meta.hasActiveState, Icon }]
  })
)

/** The props a Foundations tile passes to draw one source variant of an icon. */
function iconVariantProps(
  hasActiveState: boolean,
  variant: string | null | undefined
): { active?: boolean } {
  return hasActiveState ? { active: variant === 'active=true' } : {}
}

function Foundations({ section }: { section: string }) {
  const active: Section = sections.includes(section as Section) ? (section as Section) : 'colors'
  const foundations = data.foundations
  const fonts = foundations.textStyles.filter(
    (style, index, all) =>
      all.findIndex(
        (other) => other.fontFamily === style.fontFamily && other.fontStyle === style.fontStyle
      ) === index
  )
  const colors = foundations.tokens.filter((token) => token.type === 'COLOR')
  const spacing = foundations.tokens.filter(
    (token) => token.collection === 'Spacing' && token.name.startsWith('space/')
  )
  const radius = foundations.tokens.filter(
    (token) => token.collection === 'Spacing' && token.name.startsWith('radius/')
  )
  const layout = foundations.tokens.filter((token) => token.collection === 'Layout')
  return (
    <main className={styles.page}>
      <header>
        <h1>UI foundations</h1>
        <p>
          Captured values from the Decentraland Figma file. No replacement palette or font system.
        </p>
      </header>
      {active === 'tokens' && (
        <>
          <h2>Tokens · {foundations.tokens.length} captured mode values</h2>
          <p>
            Every captured variable, with its collection, mode and resolved value. Gradients and
            typography styles have their own tabs.
          </p>
          <div className={styles.tableScroll}>
            <table className={styles.tokenTable}>
              <thead>
                <tr>
                  <th>Collection</th>
                  <th>Mode</th>
                  <th>Token</th>
                  <th>Type</th>
                  <th>Value</th>
                  <th>CSS variable</th>
                </tr>
              </thead>
              <tbody>
                {foundations.tokens.map((token) => (
                  <tr key={token.collection + token.mode + token.name}>
                    <td>{token.collection}</td>
                    <td>{token.mode}</td>
                    <td>{token.name}</td>
                    <td>{token.type}</td>
                    <td>
                      {typeof token.value === 'number'
                        ? token.value
                        : `${token.value.hex} · ${Math.round(token.value.alpha * 100)}%`}
                    </td>
                    <td>
                      <code>{token.cssName}</code>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
      {active === 'fonts' && (
        <>
          <h2>Fonts · {fonts.length} captured family/weight combinations</h2>
          <p>
            Font files are bundled locally. Typography shows every captured size, line height and
            tracking style.
          </p>
          <div className={styles.grid}>
            {fonts.map((font) => (
              <article className={styles.card} key={font.fontFamily + font.fontStyle}>
                <strong>
                  {font.fontFamily} · {font.fontStyle}
                </strong>
                <p className={`${styles.typeSample} ${textStyles[font.className]}`}>
                  Aa Bb 0123
                  <br />
                  Decentraland
                </p>
              </article>
            ))}
          </div>
        </>
      )}
      {active === 'colors' && (
        <>
          <h2>Colors</h2>
          <div className={styles.grid}>
            {colors.map((token) => {
              if (typeof token.value === 'number') return null
              return (
                <article className={styles.card} key={token.collection + token.mode + token.name}>
                  <div
                    className={styles.swatch}
                    data-color-token={token.cssName}
                    data-expected-color={JSON.stringify(token.value)}
                    style={{ background: `var(${token.cssName})` }}
                  />
                  <strong>{token.name}</strong>
                  <small>
                    {token.collection} · {token.mode}
                  </small>
                  <code>
                    {token.value.hex} · {Math.round(token.value.alpha * 100)}%
                  </code>
                  <code>{token.cssName}</code>
                </article>
              )
            })}
          </div>
        </>
      )}
      {active === 'gradients' && (
        <>
          <h2>Gradients · {Object.keys(gradients).length} styles</h2>
          <div className={styles.grid}>
            {Object.entries(gradients).map(([name, value]) => (
              <article className={styles.card} key={name}>
                <div
                  className={styles.swatch}
                  data-gradient-token={name}
                  style={{ background: theme.gradient[name as keyof typeof theme.gradient] }}
                />
                <strong>{name.replace(/([A-Z])/g, ' $1')}</strong>
                <code>{value}</code>
                <code>{theme.gradient[name as keyof typeof theme.gradient]}</code>
              </article>
            ))}
          </div>
        </>
      )}
      {active === 'typography' && (
        <>
          <h2>Typography · {foundations.textStyles.length} styles</h2>
          <div className={styles.grid}>
            {foundations.textStyles.map((style) => (
              <article className={styles.card} key={style.name}>
                <strong>{style.name}</strong>
                <p className={`${styles.typeSample} ${textStyles[style.className]}`}>
                  Aa Bb 0123
                  <br />
                  Decentraland
                </p>
                <small>
                  {style.fontFamily} · {style.fontStyle} · {style.fontSize}px
                </small>
                <code>
                  Line height:{' '}
                  {style.lineHeightAuto
                    ? 'Auto'
                    : Math.round(style.lineHeightPercent * 100) / 100 + '%'}
                </code>
                <code>
                  Tracking: {style.letterSpacingPx}px · {style.textCase}
                </code>
              </article>
            ))}
          </div>
        </>
      )}
      {active === 'spacing' && (
        <>
          <h2>Spacing</h2>
          <div className={styles.grid}>
            {spacing.map((token) => (
              <article className={styles.card} key={token.name + token.mode}>
                <strong>{token.name}</strong>
                <code>{String(token.value)}px</code>
                <div className={styles.spacingTrack}>
                  <div
                    className={styles.spacingBar}
                    style={{ width: typeof token.value === 'number' ? token.value : 0 }}
                  />
                </div>
              </article>
            ))}
          </div>
        </>
      )}
      {active === 'borders' && (
        <>
          <h2>Corner radii</h2>
          <div className={styles.grid}>
            {radius.map((token) => (
              <article className={styles.card} key={token.name + token.mode}>
                <strong>{token.name}</strong>
                <code>{String(token.value)}px</code>
                <div
                  className={styles.radius}
                  style={{ borderRadius: typeof token.value === 'number' ? token.value : 0 }}
                />
              </article>
            ))}
          </div>
          <h2>Component border widths</h2>
          <p>Widths captured from the selected components. These are not invented global tokens.</p>
          <div className={styles.grid}>
            {foundations.strokeWidths.map((width) => (
              <article className={styles.card} key={width}>
                <code>{width}px</code>
                <div className={styles.border} style={{ borderWidth: width }} />
              </article>
            ))}
          </div>
        </>
      )}
      {active === 'layout' && (
        <>
          <h2>Breakpoints</h2>
          <div className={styles.grid}>
            {Object.entries(breakpoints).map(([name, width]) => (
              <article className={styles.card} key={name}>
                <strong>{name}</strong>
                <code>min-width: {width}px</code>
              </article>
            ))}
          </div>
          <h2>Layout and responsive modes</h2>
          <div className={styles.grid}>
            {layout.map((token) => (
              <article className={styles.card} key={token.name + token.mode}>
                <strong>{token.name}</strong>
                <small>{token.mode}</small>
                <code>{String(token.value)}px</code>
                <code>{token.cssName}</code>
              </article>
            ))}
          </div>
        </>
      )}
      {active === 'icons' && (
        <>
          <h2>Icons · {foundations.icons.length} source variants</h2>
          <div className={styles.iconGrid}>
            {foundations.icons.map((icon) => {
              const { Icon, hasActiveState = false } = iconComponents.get(icon.name) ?? {}
              return (
                <div className={styles.icon} key={icon.nodeId} title={`Figma node ${icon.nodeId}`}>
                  {Icon ? (
                    <Icon
                      size={Math.min(Math.max(icon.width, icon.height), 64)}
                      {...iconVariantProps(hasActiveState, icon.variant)}
                      aria-hidden
                    />
                  ) : (
                    <small>No generated component</small>
                  )}
                  <span>{icon.name}</span>
                  <small>{icon.variant}</small>
                </div>
              )
            })}
          </div>
        </>
      )}
      {active === 'effects' && (
        <>
          <h2>Effects · disabled</h2>
          <p>{foundations.effects.status}</p>
          <p>
            Web-modal background review is a component-scoped exception. It does not enable the
            Effects page.
          </p>
        </>
      )}
    </main>
  )
}

export { Foundations, sections }
