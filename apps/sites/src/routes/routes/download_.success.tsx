import { redirect } from "react-router";
import { canonicalSuccessHref } from "@/features/download/installers";
export function loader({ request }: { request: Request }) {
  return redirect(canonicalSuccessHref(request));
}
export default function DownloadSuccessRedirect() {
  return null;
}
