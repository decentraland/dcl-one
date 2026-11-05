// from: shop@3a322cf9:app/src/components/SkeletonCards/SkeletonCards.tsx (SkeletonCards: the design's loading card,
//       a violet-washed tile over two pill bars, pulsing)
import "./skeletoncards.css";

export default function SkeletonCards({ count }: { count: number }) {
  return (
    <>
      {Array.from({ length: Math.max(0, count) }, (_, i) => (
        <div key={i} className="shop-skel-card" aria-hidden data-testid="skeleton-card">
          <div className="shop-skel-card__thumb" />
          <div className="shop-skel-card__info">
            <div className="shop-skel-card__bar" />
            <div className="shop-skel-card__bar" data-short />
          </div>
        </div>
      ))}
    </>
  );
}
