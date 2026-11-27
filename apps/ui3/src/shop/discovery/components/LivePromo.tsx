import { lazy, Suspense, useEffect, useState } from "react";
import { Link } from "react-router";
import { LivePromo as Promo, type LivePromoProps } from "@dcl/one-ui/apps/shop/preview";
import { pickRenderer } from "../../detail/lib/pickRenderer";
import { REMOTE_FLAG, useRemoteFlagState } from "../lib/remoteFlags";

// The module path, not the "@dcl/one-ui" barrel: import() of the barrel keeps its whole namespace, which loses this
// lazy chunk and pulls every icon into the chunk the root route loads.
const AnimatedBackground = lazy(() =>
  import("@dcl/one-ui/components/AnimatedBackground/AnimatedBackground").then((m) => ({ default: m.AnimatedBackground })),
);

export default function LivePromo(props: Omit<LivePromoProps, "Link" | "backdrop" | "allowed" | "bevyFlag">) {
  const [allowed, setAllowed] = useState(false);
  const bevyFlag = useRemoteFlagState(REMOTE_FLAG.bevyAvatarPreview, allowed);
  useEffect(() => {
    if (typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    if (pickRenderer().renderer !== "full") return;
    setAllowed(true);
  }, []);

  return (
    <Promo
      {...props}
      Link={Link}
      allowed={allowed}
      bevyFlag={bevyFlag}
      backdrop={<Suspense fallback={null}><AnimatedBackground patternTiling={1} /></Suspense>}
    />
  );
}
