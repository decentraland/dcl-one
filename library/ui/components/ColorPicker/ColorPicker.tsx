import { useEffect, useRef, useState, type CSSProperties } from 'react'
import styles from './ColorPicker.module.css'
import { hexToHsv, hsvToHex, type Hsv } from './hsv'

export type ColorPickerProps = {
  /** The accessible name of the button and of the popup it opens. */
  label: string
  /** The colour, as `#rrggbb`. */
  value: string
  /** The colours offered as toggles, as `#rrggbb`. */
  presets: readonly string[]
  onChange: (hex: string) => void
  /** Only the hue bar; a hue picked there is fully saturated and bright. */
  hueOnly?: boolean
  /** The button is the colour alone, without the COLOR text and the chevron. */
  swatch?: boolean
  /**
   * Called as the popup opens, with a function that closes it; what it returns runs as the popup leaves. A
   * host registers the popup there with whatever closes its layers.
   */
  whileOpen?: (close: () => void) => () => void
}

/** How far a stepper moves a bar; the bars run from 0 to 1. */
const STEP = 0.1

/**
 * A button showing a colour that opens a light popup of preset toggles and hue, saturation and brightness
 * bars. The bars are range inputs: nothing here opens the browser's own colour picker. A press outside closes
 * the popup; it handles no Escape, which the host does through `whileOpen`.
 */
export function ColorPicker({
  label,
  value,
  presets,
  onChange,
  hueOnly = false,
  swatch = false,
  whileOpen
}: ColorPickerProps) {
  const [open, setOpen] = useState(false)
  // The bars hold their own values, so the hue survives zero saturation or brightness.
  const [hsv, setHsv] = useState<Hsv>(() => hexToHsv(value))
  const [preset, setPreset] = useState<string | null>(null)
  const root = useRef<HTMLDivElement>(null)
  const opened = useRef(whileOpen)
  useEffect(() => {
    opened.current = whileOpen
  })

  useEffect(() => {
    if (!open) return
    const close = (): void => setOpen(false)
    const press = (event: MouseEvent): void => {
      if (root.current != null && !root.current.contains(event.target as Node)) close()
    }
    document.addEventListener('mousedown', press)
    const off = opened.current?.(close)
    return () => {
      document.removeEventListener('mousedown', press)
      off?.()
    }
  }, [open])

  const toggle = (): void => {
    if (!open) {
      setHsv(hexToHsv(value))
      setPreset(presets.find((p) => p.toLowerCase() === value.toLowerCase()) ?? null)
    }
    setOpen(!open)
  }
  const pickPreset = (p: string): void => {
    setHsv(hexToHsv(p))
    setPreset(p)
    onChange(p)
  }
  const setChannel = (key: keyof Hsv, v: number): void => {
    const next = {
      ...hsv,
      [key]: key === 'h' ? Math.min(359.999, Math.max(0, v * 360)) : Math.min(1, Math.max(0, v))
    }
    if (hueOnly) {
      next.s = 1
      next.v = 1
    }
    setHsv(next)
    setPreset(null)
    onChange(hsvToHex(next))
  }
  const hueHex = hsvToHex({ h: hsv.h, s: 1, v: 1 })

  return (
    <div className={styles.root} ref={root}>
      {swatch ? (
        <button
          type="button"
          className={styles.swatch}
          aria-label={label}
          aria-expanded={open}
          onClick={toggle}
        >
          <span className={styles.swatchFill} style={{ background: value }} />
        </button>
      ) : (
        <button
          type="button"
          className={styles.trigger}
          aria-label={label}
          aria-expanded={open}
          onClick={toggle}
        >
          <span className={styles.triggerText}>COLOR</span>
          <span className={styles.preview} style={{ background: value }} />
          <span
            className={[styles.chevron, open && styles.chevronUp].filter(Boolean).join(' ')}
            aria-hidden="true"
          />
        </button>
      )}
      {open && (
        <div className={styles.popup} role="dialog" aria-label={label}>
          <div className={styles.section}>
            <span className={styles.label}>PRESETS</span>
            <div className={styles.presets} role="radiogroup" aria-label={`${label} presets`}>
              {presets.map((p) => (
                <button
                  key={p}
                  type="button"
                  role="radio"
                  aria-label={p}
                  aria-checked={p === preset}
                  className={styles.preset}
                  onClick={() => pickPreset(p)}
                >
                  <span className={styles.tint} style={{ background: p }} />
                </button>
              ))}
            </div>
          </div>
          <Bar
            label="COLOR"
            name="Hue"
            value={hsv.h / 360}
            track={styles.hueTrack}
            onChange={(v) => setChannel('h', v)}
          />
          {!hueOnly && (
            <>
              <Bar
                label="SATURATION"
                name="Saturation"
                value={hsv.s}
                track={styles.satTrack}
                trackStyle={{ '--hue': hueHex } as CSSProperties}
                onChange={(v) => setChannel('s', v)}
              />
              <Bar
                label="BRIGHTNESS"
                name="Brightness"
                value={hsv.v}
                track={styles.valTrack}
                onChange={(v) => setChannel('v', v)}
              />
            </>
          )}
        </div>
      )}
    </div>
  )
}

function Bar({
  label,
  name,
  value,
  track,
  trackStyle,
  onChange
}: {
  label: string
  name: string
  value: number
  track: string | undefined
  trackStyle?: CSSProperties
  onChange: (v: number) => void
}) {
  return (
    <div className={styles.section}>
      <span className={styles.label}>{label}</span>
      <div className={styles.barRow}>
        <button
          type="button"
          className={styles.step}
          aria-label={`Decrease ${name}`}
          disabled={value <= 0}
          onClick={() => onChange(value - STEP)}
        >
          ‹
        </button>
        <div className={[styles.track, track].filter(Boolean).join(' ')} style={trackStyle}>
          <input
            className={styles.range}
            type="range"
            min={0}
            max={1}
            step={0.001}
            aria-label={name}
            value={value}
            onChange={(event) => onChange(Number(event.target.value))}
          />
        </div>
        <button
          type="button"
          className={styles.step}
          aria-label={`Increase ${name}`}
          disabled={value >= 0.999}
          onClick={() => onChange(value + STEP)}
        >
          ›
        </button>
      </div>
    </div>
  )
}
