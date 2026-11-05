import { redirect } from "react-router";

export function loader({ request, params }: { request: Request; params: { id?: string } }) {
  const query = new URL(request.url).searchParams;
  query.set("mode", "edit");
  query.set("id", params.id ?? "");
  return redirect(`/landings/submit-hangout?${query}`);
}
