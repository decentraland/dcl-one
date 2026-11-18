import { createRoot } from 'react-dom/client'
import { Checkbox, CheckboxLabel, type CheckboxSize, type CheckboxVariant } from '@dcl/one-ui'
import '@dcl/one-ui/foundations.css'
import styles from './CheckboxPreview.module.css'

export type CheckboxMetrics = {
  x: number
  y: number
  target: number
  box: number
  glyph: number
  fill: string
  border: string
  borderWidth: string
  radius: string
  mark: string
  halo: string
  haloSize: number
  haloOffset: number
  haloDisplay: string
  check: boolean
  dash: boolean
  outline: string
  transition: string
  checked: boolean
  indeterminate: boolean
}

function metrics(id: string): CheckboxMetrics {
  const input = document.getElementById(id) as HTMLInputElement
  const root = input.parentElement!
  const [, halo, box] = Array.from(root.children) as HTMLElement[]
  const [check, dash] = Array.from(box!.children) as SVGElement[]
  const rootRect = root.getBoundingClientRect()
  const boxStyle = getComputedStyle(box!)
  const visible = (el: Element) => getComputedStyle(el).display !== 'none'
  return {
    x: rootRect.x + rootRect.width / 2,
    y: rootRect.y + rootRect.height / 2,
    target: rootRect.width,
    box: box!.getBoundingClientRect().width,
    glyph: (visible(check!) ? check! : dash!).getBoundingClientRect().width,
    fill: boxStyle.backgroundColor,
    // The stroke is an inset box shadow: "<color> 0px 0px 0px <width> inset".
    border: boxStyle.boxShadow.replace(/ 0px 0px 0px [\d.]+px inset$/, ''),
    borderWidth: /0px 0px 0px ([\d.]+px) inset$/.exec(boxStyle.boxShadow)?.[1] ?? 'none',
    radius: boxStyle.borderTopLeftRadius,
    mark: boxStyle.color,
    halo: getComputedStyle(halo!).backgroundColor,
    haloSize: halo!.getBoundingClientRect().width,
    haloOffset: halo!.getBoundingClientRect().x - rootRect.x,
    haloDisplay: getComputedStyle(halo!).display,
    check: visible(check!),
    dash: visible(dash!),
    outline: `${boxStyle.outlineStyle} ${boxStyle.outlineWidth} ${boxStyle.outlineColor}`,
    transition: boxStyle.transitionDuration,
    checked: input.checked,
    indeterminate: input.indeterminate
  }
}

Object.assign(window, { checkboxProbe: { metrics } })

const variants: CheckboxVariant[] = ['outline', 'filled']
const sizes: CheckboxSize[] = ['sm', 'md', 'lg']
const states = [
  { key: 'off', label: 'Unchecked', props: {} },
  { key: 'on', label: 'Checked', props: { defaultChecked: true } },
  { key: 'mixed', label: 'Indeterminate', props: { indeterminate: true } },
  { key: 'off-disabled', label: 'Disabled', props: { disabled: true } },
  {
    key: 'on-disabled',
    label: 'Checked, disabled',
    props: { defaultChecked: true, disabled: true }
  },
  {
    key: 'mixed-disabled',
    label: 'Mixed, disabled',
    props: { indeterminate: true, disabled: true }
  }
] as const

const cell = { padding: 8, textAlign: 'center' as const }
const edgeHelp = 'A help text long enough to need most of the width the open text may take.'

function Fixture() {
  return (
    <main
      style={{
        minHeight: '100vh',
        padding: 24,
        background: 'var(--dcl-bg-page)',
        color: 'var(--dcl-text-primary)',
        fontFamily: 'Inter, sans-serif'
      }}
    >
      <h1 style={{ fontSize: 20 }}>Checkbox</h1>
      {variants.map((variant) => (
        <section key={variant} aria-label={variant}>
          <h2 style={{ fontSize: 16 }}>variant={variant}</h2>
          <table style={{ borderCollapse: 'collapse' }}>
            <thead>
              <tr>
                <th style={cell}>size</th>
                {states.map((state) => (
                  <th key={state.key} style={{ ...cell, fontWeight: 400, fontSize: 12 }}>
                    {state.label}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {sizes.map((size) => (
                <tr key={size}>
                  <th style={cell}>{size}</th>
                  {states.map((state) => (
                    <td key={state.key} style={cell}>
                      <Checkbox
                        id={`${variant}-${size}-${state.key}`}
                        aria-label={`${variant} ${size} ${state.label}`}
                        variant={variant}
                        size={size}
                        {...state.props}
                      />
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      ))}
      <h2 style={{ fontSize: 16 }}>CheckboxLabel</h2>
      <div style={{ display: 'grid', gap: 16, justifyItems: 'start' }}>
        <CheckboxLabel
          id="label-end"
          label="Open Debug Console"
          help="Shows engine logs in a panel."
        />
        <CheckboxLabel
          id="label-start"
          label="Open Debug Console"
          labelPosition="start"
          help="Shows engine logs in a panel."
        />
        <CheckboxLabel id="label-plain" label="Without help" defaultChecked />
        <CheckboxLabel id="label-disabled" label="Disabled" disabled />
        <div style={{ width: 180 }}>
          <CheckboxLabel
            id="label-wrap"
            label="A long label that wraps inside a narrow column"
            help="Help"
          />
        </div>
      </div>
      {/* With ?edges: help buttons fixed 4px from each viewport edge, to keep the open text on
          screen. Fixed, so the page's own scrollbar never covers them. */}
      {new URLSearchParams(location.search).has('edges') && (
        <>
          <div style={{ position: 'fixed', top: 4, right: 4 }}>
            <CheckboxLabel id="label-edge-top" label="Top edge" help={edgeHelp} />
          </div>
          <div style={{ position: 'fixed', bottom: 56, right: 4 }}>
            <CheckboxLabel id="label-edge-right" label="Right edge" help={edgeHelp} />
          </div>
          <div style={{ position: 'fixed', bottom: 8, left: 4 }}>
            <CheckboxLabel id="label-edge-left" label="L" labelPosition="start" help={edgeHelp} />
          </div>
        </>
      )}
      <button
        type="button"
        id="open-dialog"
        onClick={() => document.querySelector<HTMLDialogElement>('#help-dialog')!.showModal()}
      >
        Open dialog
      </button>
      <dialog className={styles.dialog} id="help-dialog" aria-label="Settings">
        <CheckboxLabel id="label-dialog" label="In a dialog" help="Help inside a modal dialog." />
      </dialog>
    </main>
  )
}

createRoot(document.getElementById('root')!).render(<Fixture />)
