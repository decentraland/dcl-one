import { useNavigation, useRevalidator } from "react-router";
import Spinner from "@ui/atoms/Spinner";
import "./routepending.css";

/**
 * Inline in <head>: a click that leaves the app (/social, /play, /docs, or any
 * link the router did not take) loads a whole document, which useNavigation
 * never sees, so the old page sat unchanged for seconds. Marks <html> as
 * loading once the click has bubbled past React unhandled; pageshow clears it
 * for a page restored from the back/forward cache. It also starts marked: the
 * server HTML paints long before it hydrates (seconds under the dev server),
 * and clicks in between do nothing, so App clears it once hydrated.
 */
export const NAV_PENDING_SCRIPT = `(function(){var d=document.documentElement,t;
function on(){d.setAttribute("data-nav-loading","");clearTimeout(t);t=setTimeout(off,15000)}
function off(){d.removeAttribute("data-nav-loading");clearTimeout(t)}
addEventListener("click",function(e){if(e.defaultPrevented||e.button||e.metaKey||e.ctrlKey||e.shiftKey||e.altKey)return;
var a=e.target&&e.target.closest&&e.target.closest("a[href]");if(!a||a.hasAttribute("download")||(a.target&&a.target!=="_self"))return;
var u=new URL(a.href,location.href);if(u.origin!==location.origin||!/^https?:$/.test(u.protocol))return;
if(u.pathname===location.pathname&&u.search===location.search)return;on()});
addEventListener("submit",function(e){if(!e.defaultPrevented)on()});
addEventListener("pageshow",function(e){if(e.persisted)off()});on()})()`;

export default function RoutePending() {
  const navigation = useNavigation();
  const revalidator = useRevalidator();
  const pending = navigation.state !== "idle" || revalidator.state !== "idle";
  if (!pending) return null;
  const label = navigation.state === "submitting" ? "Saving changes…"
    : navigation.state === "loading" ? "Loading page…" : "Refreshing content…";
  return <>
    <div className="nav-progress" aria-hidden />
    <div className="route-pending" role="status" aria-live="polite">
      <Spinner size={20} aria-hidden />
      <span>{label}</span>
    </div>
  </>;
}
