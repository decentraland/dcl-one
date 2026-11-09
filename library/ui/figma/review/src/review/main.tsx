import '@fontsource/inter/400.css'
import '@fontsource/inter/600.css'
import '@fontsource/inter/700.css'
import '../generated/tokens/tokens.css'
import React, { useEffect, type ComponentType } from 'react'
import { createRoot } from 'react-dom/client'
import * as library from '../generated/components'
import { StatePreview } from '../components/StateMatrix'
import data from './catalog.json'
import { breakpoints } from '../generated/tokens/tokens'
import review from './review-notes.json'
import styles from './ReviewGallery.module.css'
import { useSelector } from '@tanstack/react-store'
import { InteractivePreview } from './InteractivePreview'
import { ReviewNavigation } from './ReviewNavigation'
import { matchesPlatform } from './navigation'
import { Foundations } from './Foundations'
import { propertyOptions, selectVariant } from './selection'
import { reviewStore, type CanvasMode } from './review.store'
import { reviewPageUrl } from './review-url'

const query = new URLSearchParams(location.search)
const audit = query.has('audit')
const selected =
  data.components.find((c) => c.name === query.get('component')) ??
  data.components.find((c) => c.name === 'ShopModals') ??
  data.components[0]!
const preferred = matchMedia('(max-width: 700px)').matches
  ? Math.max(
      0,
      selected.variants.findIndex((v) => /mobile/i.test(v.name))
    )
  : 0
const index = Math.min(
  Math.max(query.has('variant') ? Number(query.get('variant')) || 0 : preferred, 0),
  selected.variants.length - 1
)
const variant = selected.variants[index]!
const Component = (library as unknown as Record<string, ComponentType<Record<string, unknown>>>)[
  selected.name
]!
const finding = (review as Record<string, { status: string; finding: string }>)[selected.name]
const background =
  selected.page === 'Search Bar'
    ? '#5e5b67'
    : selected.page === 'Text Input'
      ? '#45424f'
      : selected.name.startsWith('NavMenu')
        ? '#3c3c3c'
        : '#1e1e1e'
const href = (name: string, v?: number) =>
  reviewPageUrl(
    'platform=' +
      reviewStore.state.platform +
      '&component=' +
      encodeURIComponent(name) +
      (v === undefined ? '' : '&variant=' + v)
  )

function Stage() {
  const comparison = useSelector(reviewStore, (value) => ({
    canvasMode: value.canvasMode,
    canvasWidth: value.canvasWidth
  }))
  return (
    <div
      className={styles.stage}
      id="audit-stage"
      style={{
        width:
          audit || comparison.canvasMode === 'source'
            ? Math.max(variant.width + 80, 360)
            : comparison.canvasMode === 'full'
              ? '100%'
              : comparison.canvasMode === 'custom'
                ? Math.min(7680, Math.max(160, comparison.canvasWidth))
                : breakpoints[comparison.canvasMode] || 360,
        minHeight: Math.max(variant.height + 80, 180),
        background
      }}
    >
      <StatePreview fluid={!audit && comparison.canvasMode !== 'source'}>
        <div
          id="audit-render"
          className={styles.render}
          style={!audit && comparison.canvasMode !== 'source' ? { width: '100%' } : undefined}
        >
          <Component {...variant.props} />
        </div>
      </StatePreview>
    </div>
  )
}

function CanvasControls() {
  const state = useSelector(reviewStore, (value) => value)
  useEffect(() => {
    try {
      localStorage.setItem(
        'dcl-review-canvas',
        JSON.stringify({ canvasMode: state.canvasMode, canvasWidth: state.canvasWidth })
      )
    } catch {
      /* Keep controls usable without storage. */
    }
  }, [state.canvasMode, state.canvasWidth])
  return (
    <fieldset className={styles.propertyControls}>
      <legend>Canvas width</legend>
      <label>
        Width mode
        <select
          name="canvas-width-mode"
          aria-label="Canvas width mode"
          value={state.canvasMode}
          onChange={(event) =>
            reviewStore.setState((value) => ({
              ...value,
              canvasMode: event.target.value as CanvasMode
            }))
          }
        >
          <option value="source">Figma size</option>
          <option value="full">Full width</option>
          {Object.entries(breakpoints).map(([name, width]) => (
            <option key={name} value={name}>
              {name.replace(/([A-Z])/g, ' $1')} · {width ? `${width}px` : '0+ (preview 360px)'}
            </option>
          ))}
          <option value="custom">Custom</option>
        </select>
      </label>
      {state.canvasMode === 'custom' && (
        <label>
          Pixels
          <input
            name="canvas-width"
            aria-label="Canvas width in pixels"
            type="number"
            min="160"
            max="7680"
            value={state.canvasWidth}
            onChange={(event) =>
              reviewStore.setState((value) => ({
                ...value,
                canvasWidth: Number(event.target.value)
              }))
            }
          />
        </label>
      )}
      <small>
        Presets use design-system breakpoint minimums. Mobile starts at 0 and previews at 360px.
        Fixed-size design elements keep their captured dimensions. Pixel measurements use the
        original Figma size.
      </small>
    </fieldset>
  )
}

function App() {
  const platform = useSelector(reviewStore, (value) => value.platform)
  useEffect(() => {
    void document.fonts.ready.then(() =>
      requestAnimationFrame(() =>
        requestAnimationFrame(() => {
          ;(window as unknown as { __AUDIT_READY__: boolean }).__AUDIT_READY__ = true
        })
      )
    )
  }, [])
  if (audit) return <Stage />
  return (
    <div className={styles.app}>
      <ReviewNavigation selected={selected.name} foundation={query.get('foundations')} />
      <main className={styles.main}>
        {query.has('foundations') ? (
          <Foundations section={query.get('foundations') ?? 'tokens'} />
        ) : !matchesPlatform(variant, platform) ? (
          <>
            <h1>{platform} components</h1>
            <p>
              No verified platform assignment for this selection. Choose another component or All
              platforms. Explorer has not been assigned to Bevy or Unity.
            </p>
          </>
        ) : (
          <>
            <header className={styles.header}>
              <h1>{selected.name}</h1>
              <p>
                {selected.page} · Figma node <code>{variant.id}</code>
              </p>
              <div className={styles.propertyControls}>
                {propertyOptions(
                  selected.variants.filter((item) => matchesPlatform(item, platform))
                ).map(([property, values]) => (
                  <label key={property}>
                    {property}
                    <select
                      name={`variant-${property}`}
                      aria-label={property}
                      value={(variant.properties as Record<string, string>)[property] ?? ''}
                      onChange={(event) => {
                        location.href = href(
                          selected.name,
                          selectVariant(selected.variants, index, property, event.target.value)
                        )
                      }}
                    >
                      {values.map((value) => (
                        <option key={value} value={value}>
                          {value}
                        </option>
                      ))}
                    </select>
                  </label>
                ))}
              </div>
              <p>
                Selections use existing Figma combinations. Other properties may adjust when a
                combination is unavailable.
              </p>
              <CanvasControls />
              <p>
                {variant.width} × {variant.height} · {variant.state}
              </p>
              <p className={styles.status}>
                {selected.page === 'Progress Bar' ? 'Pending owner review' : finding?.status}
              </p>
              <p>{finding?.finding}</p>
              {selected.needsManual.length > 0 && (
                <ul className={styles.issues}>
                  {selected.needsManual.map((reason) => (
                    <li key={reason}>{reason}</li>
                  ))}
                </ul>
              )}
            </header>
            <section className={styles.pair}>
              <article className={styles.panel}>
                <h2>Current component</h2>
                <div className={styles.scroll}>
                  <Stage />
                </div>
              </article>
            </section>
            <InteractivePreview
              key={selected.name + '-' + index}
              name={selected.name}
              component={Component}
              properties={variant.props}
            />
            <footer className={styles.footer}>
              These are design previews. A production component also needs real interaction,
              keyboard support and accessibility. Progress Bar remains pending owner review. Effects
              is disabled.
            </footer>
          </>
        )}
      </main>
    </div>
  )
}
createRoot(document.getElementById('root')!).render(<App />)
