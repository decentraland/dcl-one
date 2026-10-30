import { redirect } from "react-router";

export function loader({ request }: { request: Request }) {
  return redirect(`/${new URL(request.url).search}`);
}
