import "@dcl/one-ui/foundations/foundations.css";
import { data } from "react-router";

import { JumpInvalidRoute } from "@/features/jump/JumpInvalidRoute";

export function meta() {
  return [{ title: "Invalid link | Decentraland" }, { name: "robots", content: "noindex" }];
}

/** The landing renders, but answers 404 so link checkers and monitoring see a dead link. */
export function loader() {
  return data(null, { status: 404 });
}

export default function JumpPlacesInvalid() {
  return <JumpInvalidRoute kind="place" />;
}
