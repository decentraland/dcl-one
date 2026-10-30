import ChromeLink from "../../web/frames/ChromeLink";
import "../../components/surface.css";
import "./metricsnavigation.css";
import { suffixLabel } from "../../components/labelSuffix";

export default function MetricsNavigation({ active, creatorHref = "/create/metrics", scenesHref = "/create/scene-analytics", onCreator, labelSuffix }: {
  active: "creator" | "scenes";
  creatorHref?: string;
  scenesHref?: string;
  onCreator?: () => void;
  labelSuffix?: string;
}) {
  const links = [
    { id: "creator", label: "Creator", href: creatorHref },
    { id: "scenes", label: "Scenes", href: scenesHref },
  ];
  return <nav className="ch-metrics-nav" aria-label={suffixLabel("Metrics sections", labelSuffix)}>
    {links.map(({ id, label, href }) => <ChromeLink key={id} href={href}
      className={"ui-filter" + (active === id ? " is-active" : "")}
      aria-current={active === id ? "page" : undefined}
      onClick={id === "creator" && onCreator ? event => { event.preventDefault(); onCreator(); } : undefined}
    >{label}</ChromeLink>)}
  </nav>;
}
