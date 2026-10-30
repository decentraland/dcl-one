import { redirect } from "react-router";

export function loader({ request }: { request: Request }) {
  return redirect(`/landings/submit-hangout${new URL(request.url).search}`);
}
