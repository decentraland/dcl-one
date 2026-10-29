// The freshness the backend reported for an indexed list (freshness.ts): an index that is unavailable
// or still on its first pass says so, so an incomplete list never reads as "nothing for sale"; a
// lagging index says how far behind it is. A fresh, complete answer renders nothing, or with `stamp`
// a quiet "Updated N ago".

import Button from "../../../atoms/Button";
import { coarseDuration, needsNotice, type Freshness } from "../freshness";
import { t } from "../intl";
import Icon from "./Icon";
import "./indexnotice.css";

export default function IndexNotice({
  freshness,
  what,
  onRetry,
  now = Date.now(),
  stamp = false,
}: {
  freshness: Freshness | null | undefined;
  what: string;
  onRetry?: () => void;
  now?: number;
  /** Also say, quietly, when a fresh index last completed a pass. */
  stamp?: boolean;
}) {
  if (!freshness) return null;
  if (!needsNotice(freshness)) {
    if (!stamp || freshness.kind !== "fresh" || !freshness.asOf) return null;
    return (
      <p className="shop-index-stamp" data-testid="index-stamp">
        {t("freshness.asOf", { ago: `${coarseDuration(now - freshness.asOf)} ago` })}
      </p>
    );
  }
  let text: string;
  if (freshness.kind === "unavailable") {
    text = t("freshness.unavailable", { what, reason: freshness.message });
  } else if (!freshness.complete) {
    text = t("freshness.indexing", { source: freshness.source ?? what });
  } else {
    text = t("freshness.lagging", { ago: coarseDuration((freshness.lagSeconds ?? 0) * 1000) });
  }
  const asOf = freshness.kind === "fresh" && freshness.asOf ? t("freshness.asOf", { ago: `${coarseDuration(now - freshness.asOf)} ago` }) : null;
  return (
    <div className="shop-index-notice" role="status" data-testid="index-notice" data-kind={freshness.kind}>
      <Icon name="info" size={18} />
      <p>
        {text}
        {asOf ? <span className="shop-index-notice__asof"> {asOf}</span> : null}
      </p>
      {onRetry ? (
        <Button variant="secondary" size="sm" onClick={onRetry}>
          {t("freshness.retry")}
        </Button>
      ) : null}
    </div>
  );
}
