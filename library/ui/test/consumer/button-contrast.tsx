import { buttonMetrics, type TouchFeedback } from './button-metrics'
import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { Button } from '@dcl/one-ui'
import '@dcl/one-ui/foundations.css'
import styles from './button-contrast.module.css'
const probe = {
  pointerType: '',
  touchFeedback: null as TouchFeedback | null,
  metrics: () => buttonMetrics(document.querySelector<HTMLButtonElement>('#target')!)
}
Object.assign(window, { buttonProbe: probe })
function Fixture() {
  const q = new URLSearchParams(location.search),
    size = (q.get('size') ?? 'md') as 'xl' | 'lg' | 'md' | 'sm',
    viewport = q.get('viewport') === 'mobile' ? 'mobile' : 'desktop',
    variant = (q.get('variant') ?? 'primary') as
      'primary' | 'secondary' | 'dark' | 'outline' | 'outlineOnLight' | 'inverse' | 'text'
  const [count, setCount] = useState(0)
  return (
    <main className={styles.root} data-theme={q.get('theme') ?? 'dark'}>
      <button id="before" type="button">
        Before
      </button>
      <Button
        id="target"
        onPointerDown={(event) => {
          probe.pointerType = event.pointerType
        }}
        onMouseDown={(event) => {
          const native = event.nativeEvent as MouseEvent & {
            sourceCapabilities?: { firesTouchEvents?: boolean }
          }
          if (native.sourceCapabilities?.firesTouchEvents)
            probe.touchFeedback = {
              metrics: buttonMetrics(event.currentTarget),
              trusted: event.isTrusted,
              fromTouch: true,
              pointerType: probe.pointerType
            }
        }}
        leadingIcon={
          q.has('icon') ? (
            <svg viewBox="0 0 24 24">
              <path fill="currentColor" d="M4 4h16v16H4z" />
            </svg>
          ) : undefined
        }
        size={size}
        viewport={viewport}
        variant={variant}
        {...(q.has('compact')
          ? { iconOnly: true as const, 'aria-label': 'Root action' }
          : { iconOnly: false as const })}
        forceState={q.get('force') as 'hover' | 'pressed' | 'focus' | undefined}
        disabled={q.has('disabled')}
        loading={q.has('loading')}
        onClick={() => setCount((n) => n + 1)}
      >
        Root action
      </Button>
      <a id="after" href="#after">
        After
      </a>
      <output>{count}</output>
    </main>
  )
}
const failure = new URLSearchParams(location.search).get('failure')
const mount = () => {
  if (failure === 'render') throw new Error('Button fixture render rejected')
  createRoot(document.getElementById('root')!).render(
    failure === 'missing' ? <main>Missing target</main> : <Fixture />
  )
}
if (failure === 'delayed') requestAnimationFrame(() => requestAnimationFrame(mount))
else mount()
