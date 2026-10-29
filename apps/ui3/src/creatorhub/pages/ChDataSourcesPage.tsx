import Button from "../../atoms/Button";
import { PageHeader, FilterButton } from "../../components/Surface";
import { useMemo } from "react";
import SourceLedger, {
  type SourceClass,
  type SourceLedgerGroup,
} from "../components/SourceLedger";
import { PUBLIC_DATA_DISCLOSURE, formatReadStamp } from "../lib/datum";
import "./chdatasources.css";

export type SourceFilter = SourceClass | "all";

export const SOURCE_FILTERS: { id: SourceFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "live", label: "Live" },
  { id: "sampled", label: "Sampled" },
  { id: "snapshot", label: "Snapshot" },
  { id: "unavailable", label: "Unavailable" },
  { id: "unbuilt", label: "Not built" },
  { id: "excluded", label: "Excluded" },
];

type ChDataSourcesPageProps = {
  groups: readonly SourceLedgerGroup[];
  readAt?: string | null;
  filter?: SourceFilter;
  onFilterChange?: (filter: SourceFilter) => void;
  onRefresh?: () => void;
  refreshing?: boolean;
  now?: number;
};

export default function ChDataSourcesPage({
  groups,
  readAt = null,
  filter = "all",
  onFilterChange,
  onRefresh,
  refreshing = false,
  now,
}: ChDataSourcesPageProps) {
  const shown = useMemo(
    () => (filter === "all" ? groups : groups.filter((g) => g.klass === filter)),
    [groups, filter],
  );

  const total = useMemo(
    () => groups.reduce((sum, g) => sum + g.rows.length, 0),
    [groups],
  );

  const stamp = formatReadStamp(readAt, now);

  return (
    <div className="ds">
      <PageHeader
        title="Data sources"
        description={<>
            Every datum this hub can show, the endpoint behind it, and whether
            that endpoint answered just now. {total} sources.
          </>}
        actions={<div className="ds__headside">
          <Button
            variant="secondary"
            size="sm"
            onClick={onRefresh}
            disabled={refreshing || !onRefresh}
          >
            <span aria-hidden="true">&#x27F3;</span>{" "}
            {refreshing ? "Re-probing\u{2026}" : "Re-probe"}
          </Button>
          <p className="ds__stamp">
            {stamp ? `Read at ${stamp}` : "Read time unknown"}
          </p>
        </div>}
      />

      <p className="ds__disclosure" role="note">
        {PUBLIC_DATA_DISCLOSURE}
      </p>

      <div className="ds__filters" role="group" aria-label="Filter by state">
        {SOURCE_FILTERS.map((f) => {
          const count =
            f.id === "all"
              ? total
              : (groups.find((g) => g.klass === f.id)?.rows.length ?? 0);
          const active = filter === f.id;
          return (
            <FilterButton
              key={f.id}
              selected={active}
              onClick={() => onFilterChange?.(f.id)}
              disabled={!onFilterChange}
            >
              {f.label} <span className="ds__filtercount">{count}</span>
            </FilterButton>
          );
        })}
      </div>

      {shown.length === 0 ? (
        <p className="ds__none">
          No source in this ledger is in that state right now.
        </p>
      ) : (
        <SourceLedger groups={shown} headingLevel={2} now={now} />
      )}
    </div>
  );
}
