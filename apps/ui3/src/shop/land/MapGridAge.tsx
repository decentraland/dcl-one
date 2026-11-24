import { Tag } from "@dcl/one-ui";
import { useMapGridAge } from "@hud/features/map/useMapGridAge";
import { serviceHref } from "../../data/catalyst/client";

/**
 * How old the atlas under a LAND map is ("Updated 12 min ago"), from the map API's parcel grid
 * through the hook the Explorer's map page shares; nothing while that grid is fresh or its age
 * unknown. A Shop page passes it to `LandMap`'s `status`, so the map itself reads nothing.
 */
export default function MapGridAge() {
  const age = useMapGridAge(serviceHref("map"), true);
  return age == null ? null : <Tag variant="updated">{age}</Tag>;
}
