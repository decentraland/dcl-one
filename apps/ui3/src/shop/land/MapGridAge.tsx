import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { GeneralHistory } from "@dcl/one-ui";
import { fetchMapGridAge, MAP_GRID_AGE_POLL_MS, mapGridAgeLabel } from "@explorer-core/features/map/gridAge";
import { serviceHref } from "../../data/catalyst/client";
import { EnsureQueryClient } from "../market/components/kit";
import styles from "./MapGridAge.module.css";

/** This device's clock, read again every `ms` so a label computed from it keeps ageing. */
function useClock(ms: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), ms);
    return () => clearInterval(timer);
  }, [ms]);
  return now;
}

/**
 * How old the atlas under a LAND map is ("Updated 12 min ago"), from the map API's parcel grid;
 * nothing while that grid is fresh or its age unknown. The age is read through TanStack Query
 * every minute (paused in a background tab), and a failed read keeps the last answer, whose label
 * goes on ageing. It is measured on the server's clock (the answer's `Date` header,
 * `MapGridAge.skew`), so a device clock ahead or behind does not change it. A Shop page passes it
 * to `LandMap`'s `status`, so the map itself reads nothing. Every copy on a page shares one read.
 */
export default function MapGridAge() {
  return (
    <EnsureQueryClient>
      <GridAge />
    </EnsureQueryClient>
  );
}

function GridAge() {
  const base = serviceHref("map");
  const { data } = useQuery({
    queryKey: ["map", "gridAge", base],
    queryFn: ({ signal }) => fetchMapGridAge(base, signal),
    refetchInterval: MAP_GRID_AGE_POLL_MS,
    retry: false,
  });
  const now = useClock(MAP_GRID_AGE_POLL_MS);
  const age = data == null ? null : mapGridAgeLabel(data.checkedAt, now + data.skew);
  if (age == null) return null;
  return (
    <span className={styles.root}>
      <GeneralHistory className={styles.icon} size={14} focusable="false" aria-hidden="true" />
      {age}
    </span>
  );
}
