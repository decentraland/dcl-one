import { GeneralHistory } from "@dcl/one-ui";
import { useMapGridAge } from "@hud/features/map/useMapGridAge";
import { serviceHref } from "../../data/catalyst/client";
import styles from "./MapGridAge.module.css";

/**
 * How old the atlas under a LAND map is ("Updated 12 min ago"), from the map API's parcel grid
 * through the hook the Explorer's map page shares; nothing while that grid is fresh or its age
 * unknown. A Shop page passes it to `LandMap`'s `status`, so the map itself reads nothing.
 */
export default function MapGridAge() {
  const age = useMapGridAge(serviceHref("map"), true);
  if (age == null) return null;
  return (
    <span className={styles.root}>
      <GeneralHistory className={styles.icon} size={14} focusable="false" aria-hidden="true" />
      {age}
    </span>
  );
}
