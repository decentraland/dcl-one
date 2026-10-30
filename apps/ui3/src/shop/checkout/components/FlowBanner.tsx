// from: v2 ui3/src/marketplace/components/MkFlowBanner.tsx
// One banner for the Shop's flow notices: an outage explained (the price stays, the method stays,
// nothing moves to another method), a payment-rule explanation, a test-mode note, an error. Errors and
// outages are alerts; everything else is a polite status.

import type { ReactNode } from "react";

import "./flowbanner.css";

export type FlowTone = "info" | "warning" | "error" | "outage" | "success";

export default function FlowBanner({ tone = "info", title, children, action }: { tone?: FlowTone; title?: ReactNode; children?: ReactNode; action?: ReactNode }) {
  const alert = tone === "error" || tone === "outage";
  return (
    <div className="shop-flow-banner" data-tone={tone} role={alert ? "alert" : "status"}>
      <div className="shop-flow-banner__body">
        {title ? <p className="shop-flow-banner__title">{title}</p> : null}
        {children ? <div className="shop-flow-banner__text">{children}</div> : null}
      </div>
      {action ? <div className="shop-flow-banner__action">{action}</div> : null}
    </div>
  );
}
