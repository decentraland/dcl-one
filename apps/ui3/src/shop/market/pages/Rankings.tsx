// from: marketplace@3bf3662f:webapp/src/components/HomePage/HomePage.tsx (the <RankingsTable /> section, ./rankings/RankingsTable),
//       webapp/src/modules/analytics/sagas.ts (FETCH_RANKINGS).
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import { EnsureQueryClient } from "../components/kit";
import RankingsTable from "./rankings/RankingsTable";
import "./rankings/rankingstable.css";

export function RankingsView() {
  return (
    <div className="shop-rk">
      <RankingsTable />
    </div>
  );
}

export default function Rankings() {
  return (
    <EnsureQueryClient>
      <RankingsView />
    </EnsureQueryClient>
  );
}
