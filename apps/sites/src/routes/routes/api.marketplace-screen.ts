import { loader as shopScreenLoader } from "./api.screens.v1.shop";

// The native Explorer marketplace's screen-data path, now an alias of the canonical versioned
// Shop screen resource (GET /api/screens/v1/shop): the same loader and JSON. The native Shop runtime
// renders ui3's Shop pages, which read /shop-api themselves, so no route loader is proxied here.
export function loader({ request }: { request: Request }) {
  return shopScreenLoader({ request });
}
