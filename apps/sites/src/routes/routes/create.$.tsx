import { redirect } from "react-router";

/** A `/create/*` path with no page of its own, such as a retired web Creator Hub page, opens the landing. */
export function loader() {
  return redirect("/create");
}

export default function CreateFallback() {
  return null;
}
