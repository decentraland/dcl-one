import CreatorHubChrome from "@ui/creatorhub/frames/CreatorHubChrome";

import { useAuth } from "@/data/lib/auth/index";
import { openSignIn } from "@/features/components/auth/signin-store";
import { useChromeAuth } from "@ui/web/frames/chrome-auth";
import { creatorHubMeta } from "@/core/lib/seo/creator-hub-meta";

const DOWNLOAD_HREF = "/landings/creator-hub-download";
const RELEASES_HREF = "https://github.com/decentraland/creator-hub/releases";

export const meta = () => creatorHubMeta("Download Creator Hub");

export async function loader() {
  return {};
}

function DownloadGlyph() {
  return (
    <svg viewBox="0 0 24 24" width="30" height="30" aria-hidden="true">
      <path d="M11 3h2v9.2h3.3l-4.3 4.3-4.3-4.3H11V3z" fill="currentColor" />
      <path d="M5 19h14v2H5z" fill="currentColor" />
    </svg>
  );
}

export default function CreatorHubSettings() {
  const { isConnected, address } = useAuth();
  const { name } = useChromeAuth();

  return (
    <CreatorHubChrome
      active={null}
      signedIn={isConnected}
      account={address ?? ""}
      name={name}
      onSignIn={openSignIn}
    >
      <section style={CARD} aria-label="Download Creator Hub">
        <span style={BADGE}>
          <DownloadGlyph />
        </span>
        <h1 style={TITLE}>Download Creator Hub</h1>
        <p style={SUB}>
          Get the desktop app — the full toolset for building scenes, wearables
          and Worlds.
        </p>
        <a style={CTA} href={DOWNLOAD_HREF}>
          Download Creator Hub
        </a>
        <a style={LINK} href={RELEASES_HREF} target="_blank" rel="noreferrer noopener">
          View releases on GitHub
        </a>
      </section>
    </CreatorHubChrome>
  );
}

const CARD: React.CSSProperties = {
  maxWidth: 560,
  margin: "48px auto",
  padding: "48px 40px 40px",
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  textAlign: "center",
  background: "color-mix(in srgb, var(--brand) 8%, var(--fill-1))",
  border: "1px solid var(--line)",
  borderRadius: "var(--r-card)",
};
const BADGE: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  width: 64,
  height: 64,
  borderRadius: 16,
  background: "var(--brand)",
  color: "var(--on-brand)",
  marginBottom: 20,
};
const TITLE: React.CSSProperties = { margin: 0, fontSize: 28, lineHeight: 1.25 };
const SUB: React.CSSProperties = {
  margin: "10px 0 28px",
  maxWidth: 400,
  color: "var(--ink-7)",
  fontSize: 15,
  lineHeight: 1.55,
};
const CTA: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  width: "100%",
  maxWidth: 300,
  padding: "14px 24px",
  borderRadius: "var(--r-control)",
  background: "var(--brand)",
  color: "var(--on-brand)",
  fontSize: 16,
  fontWeight: 600,
  textDecoration: "none",
};
const LINK: React.CSSProperties = {
  marginTop: 16,
  color: "var(--ink-7)",
  fontSize: 13,
};