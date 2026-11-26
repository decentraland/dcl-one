import { Skeleton } from '../../components/Skeleton/Skeleton'
import styles from './ShopLoading.module.css'

/** Reserve the same artwork and caption space as the catalog cards while their data loads. */
export function ShopSkeletonCards({ count }: { count: number }) {
  return (
    <>
      {Array.from({ length: count }, (_, i) => (
        <div key={i} className={styles.card} aria-hidden="true" data-testid="skeleton-card">
          <Skeleton className={styles.thumb} />
          <div className={styles.info}>
            <Skeleton className={styles.bar} />
            <Skeleton className={styles.shortBar} />
          </div>
        </div>
      ))}
    </>
  )
}

export function ShopBadgeSkeleton({ testId }: { testId: string }) {
  return (
    <div className={styles.badge} aria-hidden="true" data-testid={testId}>
      <Skeleton className={styles.avatar} tone="neutral" />
      <Skeleton className={styles.name} tone="neutral" />
    </div>
  )
}
