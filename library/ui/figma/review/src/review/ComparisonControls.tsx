import { useEffect, useRef } from 'react'
import { useSelector } from '@tanstack/react-store'
import { reviewStore, type ComparisonMode } from './review.store'
import styles from './ComparisonControls.module.css'

function ComparisonControls({ caseKey }: { caseKey: string }) {
  const state = useSelector(reviewStore, (value) => value)
  const result = state.fidelity?.cases[caseKey]
  return (
    <section className={styles.controls} aria-label="Visual comparison">
      <label>
        View
        <select
          name="comparison-view"
          value={state.mode}
          onChange={(event) =>
            reviewStore.setState((value) => ({
              ...value,
              mode: event.target.value as ComparisonMode
            }))
          }
        >
          <option value="side-by-side">Side by side</option>
          <option value="overlay">Live overlay</option>
          <option value="difference">Pixel difference</option>
        </select>
      </label>
      {state.mode === 'overlay' && (
        <label>
          Figma opacity · {Math.round(state.opacity * 100)}%
          <input
            name="figma-opacity"
            type="range"
            min="0"
            max="1"
            step="0.01"
            value={state.opacity}
            onChange={(event) =>
              reviewStore.setState((value) => ({ ...value, opacity: Number(event.target.value) }))
            }
          />
        </label>
      )}
      <p>
        {!result
          ? 'Pixel measurement pending.'
          : result.status === 'blocked'
            ? result.reason
            : result.differentPixels === 0
              ? 'Exact in the captured 1× render.'
              : `${result.differencePercent.toFixed(3)}% of pixels differ. Visual approval pending.`}
      </p>
      <small>
        Overlay uses the live component. Pixel difference compares the recorded Chromium render
        against the original Figma image at 1×. Font rasterization can vary across devices.
      </small>
    </section>
  )
}

function PixelDifference({ caseKey, source }: { caseKey: string; source: string }) {
  const result = useSelector(reviewStore, (value) => value.fidelity?.cases[caseKey])
  const canvas = useRef<HTMLCanvasElement>(null)
  useEffect(() => {
    if (!result?.image || result.status === 'blocked') return
    let active = true
    const original = new Image()
    const actual = new Image()
    original.src = source
    actual.src = result.image
    void Promise.all([original.decode(), actual.decode()])
      .then(() => {
        if (!active || !canvas.current) return
        const output = canvas.current
        output.width = original.naturalWidth
        output.height = original.naturalHeight
        const context = output.getContext('2d')
        if (
          !context ||
          actual.naturalWidth !== output.width ||
          actual.naturalHeight !== output.height
        )
          return
        context.fillStyle = '#1e1e1e'
        context.fillRect(0, 0, output.width, output.height)
        context.drawImage(original, 0, 0)
        const expected = context.getImageData(0, 0, output.width, output.height)
        context.fillRect(0, 0, output.width, output.height)
        context.drawImage(actual, 0, 0)
        const rendered = context.getImageData(0, 0, output.width, output.height)
        for (let index = 0; index < expected.data.length; index += 4) {
          const difference = Math.max(
            ...[0, 1, 2].map((channel) =>
              Math.abs(expected.data[index + channel]! - rendered.data[index + channel]!)
            )
          )
          rendered.data[index] = difference ? 255 : 20
          rendered.data[index + 1] = difference ? Math.max(0, 90 - difference) : 20
          rendered.data[index + 2] = difference ? 120 : 20
          rendered.data[index + 3] = 255
        }
        context.putImageData(rendered, 0, 0)
      })
      .catch(() => undefined)
    return () => {
      active = false
    }
  }, [result, source])
  if (!result || result.status === 'blocked')
    return <p>Pixel capture is pending for this variant.</p>
  return (
    <div className={styles.diff}>
      <p>Pink pixels differ. Dark pixels match exactly.</p>
      <canvas
        ref={canvas}
        role="img"
        aria-label="Pixel difference against the original Figma variant"
      />
    </div>
  )
}

export { ComparisonControls, PixelDifference }
