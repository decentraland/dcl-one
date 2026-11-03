import { useEffect, useRef, useState } from 'react'
import { Button, ModalShell, Select, TextInput, Toggle, openPopup } from '../../design'
import styles from './DebugPanel.module.css'

type RunCommand = (line: string) => Promise<string>

// Serialized by console::DebugSection. Fields name paths in the owning plugin's JSON reply.
export interface DebugSection {
  title: string
  query: string
  fields: [label: string, path: string][]
  actions: [label: string, command: string][]
  inputs?: WidgetInput[]
}

interface WidgetInput {
  label: string
  field: string
  command: string
  kind: { type: 'text' | 'toggle' } | { type: 'number'; options: { min: number; max: number; step: number } }
    | { type: 'select'; options: [label: string, argument: string][] }
    | { type: 'actionButton'; options: { enable: [label: string, argument: string]; disable: [label: string, argument: string] } }
}

export function useCommandResult<T>(run: RunCommand, line: string, repeat: boolean) {
  const [value, setValue] = useState<T>()
  const [error, setError] = useState('')
  const refresh = useRef<() => Promise<void>>(undefined)
  useEffect(() => {
    let active = true
    let latest = 0
    let timer: ReturnType<typeof setTimeout>
    async function read() {
      // A command readback supersedes any poll that was already in flight.
      const request = ++latest
      clearTimeout(timer)
      try {
        const next: T = JSON.parse(await run(line))
        if (active && request === latest) {
          setValue(next)
          setError('')
        }
      } catch (e) {
        if (active && request === latest) setError(String(e))
      } finally {
        if (active && repeat && request === latest) timer = setTimeout(read, 1000)
      }
    }
    refresh.current = read
    void read()
    return () => { active = false; clearTimeout(timer); refresh.current = undefined }
  }, [line, repeat, run])
  return { value, error, refresh: () => refresh.current?.() }
}

function fieldValue(data: unknown, path: string): unknown {
  let value = data
  for (const key of path.split('.')) {
    if (value == null || typeof value !== 'object' || !Object.hasOwn(value, key)) return undefined
    value = (value as Record<string, unknown>)[key]
  }
  return value
}

// Plugins can explain missing values by path, by enclosing object, or with a section default.
function unavailableReason(reasons: unknown, path: string): string | undefined {
  if (!reasons || typeof reasons !== 'object') return
  const entries = reasons as Record<string, unknown>
  for (;;) {
    const reason = entries[path]
    if (Object.hasOwn(entries, path) && typeof reason === 'string') return reason
    if (!path) return
    const dot = path.lastIndexOf('.')
    path = dot < 0 ? '' : path.slice(0, dot)
  }
}

function displayField(value: unknown): string {
  if (value == null) return 'Unavailable'
  if (typeof value === 'boolean') return value ? 'Yes' : 'No'
  if (typeof value === 'number') return String(Number(value.toFixed(2)))
  return typeof value === 'object' ? JSON.stringify(value, null, 2) : String(value)
}

function NumericValues({ label, items }: {
  label: string; items: { caption?: string; value: number | null; unit?: string }[]
}) {
  return <ul className={styles.numericValues} role="list" aria-label={label}>
    {items.map(({ caption, value, unit }, index) => <li key={index}>
      {caption && <span className={styles.caption}>{caption} </span>}
      <span>{value === null ? 'Unavailable' : displayField(value)}{unit && ' ' + unit}</span>
    </li>)}
  </ul>
}

function numericTuple(value: unknown): value is (number | null)[] {
  return Array.isArray(value) && value.length > 0 && value.every(v => v === null || Number.isFinite(v))
}

function StructuredReading({ value, label }: { value: object; label: string }) {
  const [open, setOpen] = useState(false)
  const count = Object.keys(value).length
  if (count === 0) return <>None</>
  return (
    <>
      <Button size="sm" variant="ghost" aria-label={label + ' details'} aria-expanded={open} onClick={() => setOpen(!open)}>
        {count} {count === 1 ? 'entry' : 'entries'} {open ? '▴' : '▾'}
      </Button>
      {open && <pre className={styles.reply}>{JSON.stringify(value, null, 2)}</pre>}
    </>
  )
}

function FieldReading({ value, label, captions }: { value: unknown; label: string; captions?: string[] }) {
  if (numericTuple(value)) return <NumericValues label={label}
    items={value.map((n, index) => ({ value: n, caption: captions?.[index] }))} />
  if (value && typeof value === 'object' && 'samples' in value && 'unit' in value &&
    typeof value.unit === 'string' && Array.isArray(value.samples) && value.samples.every(Number.isFinite)) {
    const samples: number[] = value.samples
    if (!samples.length) return <>Waiting for samples…</>
    const min = Math.min(...samples)
    const max = Math.max(...samples)
    const points = samples.map((n, i) =>
      [100 * i / Math.max(1, samples.length - 1), 30 - 28 * (n - min) / (max - min || 1)].join(',')).join(' ')
    return <figure className={styles.chart}>
      <svg viewBox="0 0 100 32" preserveAspectRatio="none" role="img" aria-label={label + ' history'}>
        <polyline points={points} fill="none" stroke="currentColor" vectorEffect="non-scaling-stroke" />
      </svg>
      <figcaption><NumericValues label={label + ' summary'} items={[
        { caption: 'min', value: min, unit: value.unit }, { caption: 'max', value: max, unit: value.unit },
        { caption: 'samples', value: samples.length }
      ]} /></figcaption>
    </figure>
  }
  if (value !== null && typeof value === 'object') {
    return <StructuredReading value={value} label={label} />
  }
  return <>{displayField(value)}</>
}

function Readings({ fields, data }: { fields: DebugSection['fields']; data: unknown }) {
  const reasons = fieldValue(data, 'unavailable')
  const groups = new Map<string, string[]>()
  const rows = fields.map(([label, path]) => {
    const value = fieldValue(data, path)
    const reason = value == null ? unavailableReason(reasons, path) : undefined
    if (reason) {
      const labels = groups.get(reason)
      if (labels) labels.push(label)
      else groups.set(reason, [label])
    }
    return { label, value, reason }
  })
  return <dl className={styles.fields}>{rows.map(({ label, value, reason }) => {
    const labels = reason && groups.get(reason)
    if (labels && labels[0] !== label) return null
    const suffix = numericTuple(value) ? / \(([^()]+ \/ [^()]+)\)$/.exec(label) : null
    const parts = suffix?.[1].split(' / ')
    const labelled = parts && Array.isArray(value) && parts.length === value.length
    const title = labelled && suffix ? label.slice(0, suffix.index) : label
    return <div key={label}>
      <dt>{labels && labels.length > 1
        ? <details><summary>{labels.length} readings</summary>{labels.join(', ')}</details>
        : title}</dt>
      <dd>{reason ?? <FieldReading value={value} label={title} captions={labelled ? parts : undefined} />}</dd>
    </div>
  })}</dl>
}

function InputWidget({ input, live, reason, busy, submit }: {
  input: WidgetInput; live: unknown; reason?: string; busy: boolean; submit: (line: string) => Promise<boolean>
}) {
  const [draft, setDraft] = useState<string | null>(null)
  const apply = (v: string) => submit(input.command.replace('{value}', () => v).trimEnd())
  const kind = input.kind
  const unavailable = input.field !== '' && live == null
  if (kind.type === 'actionButton') {
    const [caption, argument] = live === true ? kind.options.disable : kind.options.enable
    return <div className={styles.inputRow}>
      <span>{input.label}{unavailable && reason && <> — {reason}</>}</span>
      <Button size="sm" variant="secondary" disabled={busy || typeof live !== 'boolean'}
        onClick={() => void apply(argument)}>{caption}</Button>
    </div>
  }
  if (kind.type === 'toggle') return (
    <div className={styles.inputRow}><span>{input.label}{unavailable && reason && <> — {reason}</>}</span>
      <Toggle aria-label={input.label} checked={live === true} disabled={busy || unavailable}
        onChange={(v) => void apply(String(v))} /></div>
  )
  if (kind.type === 'select') return (
    <div className={styles.inputRow}><span>{input.label}{unavailable && reason && <> — {reason}</>}</span>
      <Select aria-label={input.label} value={String(live ?? '')} disabled={busy || unavailable}
        options={kind.options.map(([label, value]) => ({ label, value }))}
        onChange={(v) => void apply(v)} /></div>
  )
  const numeric = kind.type === 'number'
  // Seed the editor on its HTML step grid. Rust f32 values and a running sky clock
  // can land between steps, which otherwise makes an untouched form invalid.
  let initial = String(live ?? '')
  if (kind.type === 'number' && typeof live === 'number') {
    const { min, max, step } = kind.options
    const steps = Math.round((live - min) / step)
    initial = String(Number(Math.min(max, Math.max(min, min + steps * step)).toPrecision(12)))
  }
  const value = draft ?? initial
  return (
    <form className={styles.edit} onSubmit={async (event) => {
      event.preventDefault()
      if (await apply(value.trim())) setDraft(null)
    }}>
      <label>{input.label}
        <TextInput type={numeric ? 'number' : 'text'} value={value} onChange={setDraft}
          {...(kind.type === 'number' ? kind.options : {})} required disabled={busy} />
      </label>
      <Button type="submit" size="sm" variant="secondary" aria-label={'Apply ' + input.label}
        disabled={busy || !value.trim()}>Apply</Button>
    </form>
  )
}

function Widgets({ section, run }: { section: DebugSection; run: RunCommand }) {
  const { value, error, refresh } = useCommandResult<unknown>(run, section.query, true)
  const [busy, setBusy] = useState(false)
  const [result, setResult] = useState('')
  async function invoke(line: string) {
    setBusy(true)
    setResult('')
    try {
      setResult(await run(line) || 'Applied')
      return true
    } catch (e) {
      setResult(String(e))
      return false
    } finally {
      await refresh()
      setBusy(false)
    }
  }

  return (
    <>
      {error ? <p role="status">Could not read {section.title}. {error}</p> :
        value === undefined ? <p>Reading…</p> :
          <Readings fields={section.fields} data={value} />}
      {section.actions.length > 0 && (
        <div className={styles.actions}>
          {section.actions.map(([label, line]) => (
            <Button key={label} size="sm" variant="secondary" disabled={busy} onClick={() => void invoke(line)}>
              {label}
            </Button>
          ))}
        </div>
      )}
      {section.inputs?.map((input) => (
        <InputWidget key={input.label} input={input} live={error ? undefined : fieldValue(value, input.field)}
          reason={unavailableReason(fieldValue(value, 'unavailable'), input.field)} busy={busy} submit={invoke} />
      ))}
      {result && <pre className={styles.reply} role="status">{result}</pre>}
    </>
  )
}

function SectionRow({ section, run, open, toggle }: {
  section: DebugSection; run: RunCommand; open: boolean; toggle: () => void
}) {
  return (
    <section className={styles.section}>
      <Button size="sm" variant="ghost" aria-expanded={open} onClick={toggle}>
        <span aria-hidden="true">{open ? '▾' : '▸'}</span> {section.title}
      </Button>
      {open && <Widgets section={section} run={run} />}
    </section>
  )
}

function DebugPanel({ run, close, initial }: { run: RunCommand; close: () => void; initial: string }) {
  const { value: sections, error } = useCommandResult<DebugSection[]>(run, '/debug_panel', false)
  const [filter, setFilter] = useState(initial)
  const [expanded, setExpanded] = useState<string[]>(initial ? [initial.toLowerCase()] : [])
  const isOpen = (s: DebugSection) => expanded.includes(s.title.toLowerCase())
  const toggle = (s: DebugSection) => {
    const id = s.title.toLowerCase()
    setExpanded((open) => open.includes(id) ? open.filter((t) => t !== id) : [...open, id])
  }
  const term = filter.trim().toLowerCase()
  const matches = sections?.filter((s) => [s.title, ...s.fields.map(([label]) => label),
    ...s.actions.map(([label]) => label), ...(s.inputs ?? []).flatMap(i => i.kind.type === 'actionButton'
      ? [i.label, i.kind.options.enable[0], i.kind.options.disable[0]] : [i.label])].some((label) => label.toLowerCase().includes(term)))
  return (
    <ModalShell title="Debug panel" ariaLabel="Debug panel" onClose={close} className={styles.panel}>
      <TextInput type="search" aria-label="Find debug controls" placeholder="Find a section or control"
        value={filter} onChange={setFilter} />
      {error ? <p role="status">Could not load debug sections. {error}</p> :
        !sections ? <p>Loading debug sections…</p> :
        sections.length === 0 ? <p>No plugins registered debug sections.</p> :
        matches?.length === 0 ? <p>No matching controls.</p> :
          matches?.map((section) =>
            <SectionRow key={section.title} section={section} run={run} open={isOpen(section)} toggle={() => toggle(section)} />)}
    </ModalShell>
  )
}

export function openDebugPanel(run: RunCommand, initial: string, onClose: () => void,
  launcherLayer: (element: HTMLDivElement | null) => void): () => void {
  return openPopup((close) => <>
    <div className={styles.launcherLayer} ref={launcherLayer} />
    <DebugPanel run={run} close={close} initial={initial} />
  </>, { dim: false, onClose })
}
