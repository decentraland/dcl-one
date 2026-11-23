import { Link } from "react-router";

import ExFlowMapPage from "@dcl/one-ui/apps/sites/flow-map/ExFlowMapPage";

export const meta = () => [{ title: "Flow map \u{2014} Explorer | Decentraland" }];

export default function ExplorerMapRoute() {
  return <ExFlowMapPage LinkComponent={Link} />;
}
