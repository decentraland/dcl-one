import type { LoadingEvents } from "./events/loading";
import type { ClientEvents } from "./events/client";
import type { CreatorHubEvents } from "./events/creator-hub";
import type { LandingsEvents } from "./events/landings";
import type { MarketplaceEvents } from "./events/marketplace";
import type { MiscEvents } from "./events/misc";

export type TelemetryEvents = LoadingEvents &
  ClientEvents &
  CreatorHubEvents &
  LandingsEvents &
  MarketplaceEvents &
  MiscEvents;

export type TelemetryEventName = keyof TelemetryEvents;
