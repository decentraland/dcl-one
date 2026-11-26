// Reads the next page when a list's end comes into view: the browse grid and the announcements.
import { useEffect, useRef } from 'react'
import styles from './LoadMore.module.css'

// The observer is made again whenever the list grows, so a page that leaves the end still in view
// reads the next one too. jsdom has no IntersectionObserver: the list then shows its first page only.
export function LoadMore({ count, onVisible }: { count: number; onVisible: () => void }): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null)
  const visible = useRef(onVisible)
  visible.current = onVisible
  useEffect(() => {
    const el = ref.current
    if (el == null || typeof IntersectionObserver === 'undefined') return
    const io = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) visible.current()
    }, { rootMargin: '0px 0px 200px 0px' })
    io.observe(el)
    return () => io.disconnect()
  }, [count])
  return <div ref={ref} className={styles.loadMore} aria-hidden="true" />
}
