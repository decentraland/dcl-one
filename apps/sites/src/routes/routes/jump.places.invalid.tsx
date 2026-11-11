import "@dcl/one-ui/foundations/foundations.css";

import { JumpInvalidRoute } from "@/features/jump/JumpInvalidRoute";

export function meta() {
  return [{ title: "Invalid link | Decentraland" }, { name: "robots", content: "noindex" }];
}

export default function JumpPlacesInvalid() {
  return <JumpInvalidRoute kind="place" />;
}
