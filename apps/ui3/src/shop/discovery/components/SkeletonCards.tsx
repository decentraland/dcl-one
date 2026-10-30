// from: shop@94584a5c:app/src/components/SkeletonCards/SkeletonCards.tsx
import "./skeletoncards.css";

export default function SkeletonCards({ count }: { count: number }) {
  return (
    <>
      {Array.from({ length: Math.max(0, count) }, (_, i) => (
        <div key={i} className="shop-skel-card" aria-hidden data-testid="skeleton-card">
          <div className="shop-skel-card__media" />
          <div className="shop-skel-card__line" />
          <div className="shop-skel-card__line shop-skel-card__line--short" />
        </div>
      ))}
    </>
  );
}
