import type { ComponentType, ReactNode } from 'react'
import { breakpoints, layout, textClasses, theme } from '@dcl/one-ui'
import * as foundations from '@dcl/one-ui/foundations'
import type { IconProps } from '@dcl/one-ui/foundations'
import { sortByLabel } from '../order'
import styles from './FoundationPages.module.css'

export type FoundationPage = {
  name: string
  title: string
  summary: string
  render: () => ReactNode
}

function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className={styles.group}>
      <h3 className={textClasses.bodyMdStrong}>{title}</h3>
      {children}
    </section>
  )
}

function Swatches({ values }: { values: Readonly<Record<string, string>> }) {
  return (
    <ul className={styles.swatches}>
      {Object.entries(values).map(([name, value]) => (
        <li key={name} className={styles.swatch}>
          <span className={styles.chip} style={{ background: value }} />
          <code className={styles.label}>{name}</code>
        </li>
      ))}
    </ul>
  )
}

const icons = foundations.iconComponentNames.map(
  (name) =>
    [name, (foundations as unknown as Record<string, ComponentType<IconProps>>)[name]!] as const
)

const pages: readonly FoundationPage[] = [
  {
    name: 'colors',
    title: 'Colors',
    summary: 'Semantic colours (theme.color), then the primitive palette and gradients.',
    render: () => (
      <>
        {Object.entries(theme.color).map(([group, values]) => (
          <Group key={group} title={`color.${group}`}>
            <Swatches values={values} />
          </Group>
        ))}
        {Object.entries(theme.palette).map(([group, values]) => (
          <Group key={group} title={`palette.${group}`}>
            <Swatches values={values} />
          </Group>
        ))}
        <Group title="gradient">
          <Swatches values={theme.gradient} />
        </Group>
      </>
    )
  },
  {
    name: 'typography',
    title: 'Typography',
    summary: 'Every text style class (textClasses); responsive sizes follow the canvas width.',
    render: () => (
      <ul className={styles.typeList}>
        {Object.entries(textClasses).map(([name, className]) => (
          <li key={name}>
            <code className={styles.label}>{name}</code>
            <p className={className}>The quick brown fox jumps over the lazy dog</p>
          </li>
        ))}
      </ul>
    )
  },
  {
    name: 'spacing',
    title: 'Spacing and radius',
    summary: 'The spacing scale (theme.space) and corner radii (theme.radius).',
    render: () => (
      <>
        <Group title="space">
          <ul className={styles.scale}>
            {Object.entries(theme.space).map(([name, value]) => (
              <li key={name}>
                <span className={styles.bar} style={{ inlineSize: value }} />
                <code className={styles.label}>{name}</code>
              </li>
            ))}
          </ul>
        </Group>
        <Group title="radius">
          <ul className={styles.swatches}>
            {Object.entries(theme.radius).map(([name, value]) => (
              <li key={name} className={styles.swatch}>
                <span className={styles.chip} style={{ borderRadius: value }} />
                <code className={styles.label}>{name}</code>
              </li>
            ))}
          </ul>
        </Group>
      </>
    )
  },
  {
    name: 'layout',
    title: 'Layout',
    summary: 'Breakpoints and the responsive layout values the canvas width selects.',
    render: () => (
      <>
        <Group title="breakpoints (min width, px)">
          <dl className={styles.values}>
            {Object.entries(breakpoints).map(([name, value]) => (
              <div key={name}>
                <dt>{name}</dt>
                <dd>{value}</dd>
              </div>
            ))}
          </dl>
        </Group>
        <Group title="layout (px per breakpoint)">
          <dl className={styles.values}>
            {Object.entries(layout).map(([name, values]) => (
              <div key={name}>
                <dt>{name}</dt>
                <dd>{Object.values(values).join(' / ')}</dd>
              </div>
            ))}
          </dl>
        </Group>
        <Group title="margin at this width">
          <div className={styles.margin}>
            <span>content column</span>
          </div>
        </Group>
      </>
    )
  },
  {
    name: 'icons',
    title: 'Icons',
    summary: `All ${icons.length} icon components, at 24px.`,
    render: () => (
      <ul className={styles.icons}>
        {icons.map(([name, Icon]) => (
          <li key={name}>
            <Icon size={24} aria-hidden="true" />
            <code className={styles.label}>{name}</code>
          </li>
        ))}
      </ul>
    )
  }
]

/** The foundations pages by title (`sortByLabel`), the label the sidebar lists them under. */
export const foundationPages: readonly FoundationPage[] = sortByLabel(pages, (page) => page.title)
