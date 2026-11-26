import { ShopCardSkeleton } from '../../components/ShopCard/ShopCard'
import { Skeleton } from '../../components/Skeleton/Skeleton'
import styles from './ShopLoading.module.css'

/** Reserve the same artwork and caption space as the catalog cards while their data loads. */
export function ShopSkeletonCards({ count }: { count: number }) {
  return (
    <>
      {Array.from({ length: count }, (_, i) => (
        <ShopCardSkeleton key={i} variant="catalogue" data-testid="skeleton-card" />
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
