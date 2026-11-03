import { useEffect, useState } from 'react'
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
}

export function useCommandResult<T>(run: RunCommand, line: string, repeat: boolean) {
  const [value, setValue] = useState<T>()
  const [error, setError] = useState('')
  useEffect(() => {
    let active = true
    let timer: ReturnType<typeof setTimeout>
    async function read() {
      try {
        const next: T = JSON.parse(await run(line))
        if (active) {
          setValue(next)
          setError('')
        }
      } catch (e) {
        if (active) setError(String(e))
      } finally {
        if (active && repeat) timer = setTimeout(read, 1000)
      }
    }
    void read()
    return () => { active = false; clearTimeout(timer) }
  }, [line, repeat, run])
  return { value, error }
}

function fieldValue(data: unknown, path: string): unknown {
  let value = data
  for (const key of path.split('.')) {
    if (value == null || typeof value !== 'object' || !Object.hasOwn(value, key)) return undefined
    value = (value as Record<string, unknown>)[key]
  }
  return value
}

function displayField(value: unknown): string {
  if (value == null) return 'Unavailable'
  if (typeof value === 'boolean') return value ? 'Yes' : 'No'
  if (typeof value === 'number') return String(Number(value.toFixed(2)))
  if (Array.isArray(value) && value.every((v) => typeof v === 'number')) return value.map(displayField).join(', ')
  return typeof value === 'object' ? JSON.stringify(value, null, 2) : String(value)
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

function FieldReading({ value, label }: { value: unknown; label: string }) {
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
      <figcaption>{displayField(min)}–{displayField(max)} {value.unit} · {samples.length} samples</figcaption>
    </figure>
  }
  if (value !== null && typeof value === 'object' && !(Array.isArray(value) && value.length > 0 && value.every(Number.isFinite))) {
    return <StructuredReading value={value} label={label} />
  }
  return <>{displayField(value)}</>
}

function InputWidget({ input, live, busy, submit }: {
  input: WidgetInput; live: unknown; busy: boolean; submit: (line: string) => Promise<boolean>
}) {
  const [draft, setDraft] = useState<string | null>(null)
  const apply = (v: string) => submit(input.command.replace('{value}', () => v))
  const kind = input.kind
  const unavailable = input.field !== '' && live == null
  if (kind.type === 'toggle') return (
    <div className={styles.inputRow}><span>{input.label}</span>
      <Toggle aria-label={input.label} checked={live === true} disabled={busy || unavailable}
        onChange={(v) => void apply(String(v))} /></div>
  )
  if (kind.type === 'select') return (
    <div className={styles.inputRow}><span>{input.label}</span>
      <Select aria-label={input.label} value={String(live ?? '')} disabled={busy || unavailable}
        options={kind.options.map(([label, value]) => ({ label, value }))}
        onChange={(v) => void apply(v)} /></div>
  )
  const numeric = kind.type === 'number'
  const value = draft ?? String(live ?? '')
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
  const { value, error } = useCommandResult<unknown>(run, section.query, true)
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
      setBusy(false)
    }
  }

  return (
    <>
      {error ? <p role="status">Could not read {section.title}. {error}</p> :
        value === undefined ? <p>Reading…</p> :
          <dl className={styles.fields}>
            {section.fields.map(([label, path]) => (
              <div key={label}><dt>{label}</dt><dd><FieldReading value={fieldValue(value, path)} label={label} /></dd></div>
            ))}
          </dl>}
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
        <InputWidget key={input.label} input={input} live={fieldValue(value, input.field)} busy={busy} submit={invoke} />
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
    ...s.actions.map(([label]) => label), ...(s.inputs ?? []).map((i) => i.label)].some((label) => label.toLowerCase().includes(term)))
  return (
    <ModalShell title="Debug panel" ariaLabel="Debug panel" onClose={close} className={styles.panel}>
      <TextInput type="search" aria-label="Find debug controls" placeholder="Find a section or control"
        value={filter} onChange={setFilter} />
      {error ? <p role="status">Could not load debug sections. {error}</p> :
        !sections ? <p>Loading debug sections…</p> :
        sections.length === 0 ? <p>No plugins registered debug sections.</p> :
        matches?.length === 0 ? <p>No matching controls.</p> :
          matches?.sort((a, b) => Number(isOpen(b)) - Number(isOpen(a))).map((section) =>
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
