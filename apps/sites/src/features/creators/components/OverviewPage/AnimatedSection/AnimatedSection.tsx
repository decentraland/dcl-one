import type { ReactNode } from 'react'
import { useInView } from 'react-intersection-observer'
import { useSectionViewed } from '~/hooks/useSectionViewed'
import type { OverviewSection } from '~/lib/overviewAnalytics'
import styles from './AnimatedSection.module.css'

type AnimatedSectionProps = {
  section: OverviewSection
  children: ReactNode
  /** How much of the section must be visible before it reveals; also when `Section Viewed` fires. */
  threshold?: number
}

/** Slides its content into view the first time it is scrolled to, and reports the section as viewed. */
const AnimatedSection = ({ section, children, threshold = 0.1 }: AnimatedSectionProps) => {
  const { ref, inView } = useInView({ triggerOnce: true, threshold })
  useSectionViewed(section, inView)
  return (
    <div
      className={styles.reveal}
      ref={ref}
      data-testid={`overview-section-${section}`}
      data-visible={inView || undefined}
    >
      {children}
    </div>
  )
}

export { AnimatedSection }
