import { useLayoutEffect, useRef, type ReactNode } from 'react'
import styles from './StateMatrix.module.css'
import type { ForcedState } from './figma-view-state'

type MatrixCase = {
  key: string
  props: Record<string, unknown>
  states: ('base' | ForcedState)[]
}

type MatrixProps = { component: (props: never) => ReactNode; cases: MatrixCase[] }

function StatePreview({ children, fluid = false }: { children: ReactNode; fluid?: boolean }) {
  const frameRef = useRef<HTMLDivElement>(null)
  const contentRef = useRef<HTMLDivElement>(null)

  useLayoutEffect(() => {
    const frame = frameRef.current
    const content = contentRef.current
    if (!frame || !content) return
    let active = true
    let pending = 0
    const measure = () => {
      if (!active) return
      const bounds = content.getBoundingClientRect()
      let left = bounds.left
      let top = bounds.top
      let right = bounds.right
      let bottom = bounds.bottom
      for (const child of content.querySelectorAll('*')) {
        const rect = child.getBoundingClientRect()
        if (
          rect.width === 0 ||
          rect.height === 0 ||
          getComputedStyle(child).visibility === 'hidden'
        )
          continue
        left = Math.min(left, rect.left)
        top = Math.min(top, rect.top)
        right = Math.max(right, rect.right)
        bottom = Math.max(bottom, rect.bottom)
      }
      frame.style.paddingRight = `${Math.ceil(right - bounds.right)}px`
      frame.style.paddingBottom = `${Math.ceil(bottom - bounds.bottom)}px`
      frame.style.paddingLeft = `${Math.ceil(bounds.left - left)}px`
      frame.style.paddingTop = `${Math.ceil(bounds.top - top)}px`
    }
    const observer = new ResizeObserver(() => {
      cancelAnimationFrame(pending)
      pending = requestAnimationFrame(measure)
    })
    observer.observe(content)
    content.querySelectorAll('*').forEach((child) => observer.observe(child))
    measure()
    void document.fonts.ready.then(measure)
    return () => {
      active = false
      observer.disconnect()
      cancelAnimationFrame(pending)
    }
  }, [children])

  return (
    <div
      ref={frameRef}
      className={styles.previewFrame}
      style={fluid ? { width: '100%' } : undefined}
      data-state-preview
    >
      <div
        ref={contentRef}
        className={styles.previewContent}
        style={fluid ? { width: '100%' } : undefined}
      >
        {children}
      </div>
    </div>
  )
}

function StateMatrix({ component, cases }: MatrixProps) {
  const Component = component as unknown as (props: Record<string, unknown>) => ReactNode
  return (
    <div className={styles.grid}>
      {cases.map((entry) => (
        <div className={styles.row} key={entry.key}>
          <code className={styles.title}>{entry.key}</code>
          <div className={styles.cells}>
            {entry.states.map((state) => (
              <div className={styles.cell} key={state}>
                <span className={styles.caption}>{state}</span>
                <StatePreview>
                  <Component {...entry.props} forceState={state === 'base' ? undefined : state} />
                </StatePreview>
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  )
}

export { StateMatrix, StatePreview }
export type { MatrixCase }
