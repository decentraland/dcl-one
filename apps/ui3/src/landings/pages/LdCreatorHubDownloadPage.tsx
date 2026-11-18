import type { MouseEvent } from "react";

import StCreatorHubDownload, { ICON } from "../../web/pages/StCreatorHubDownload";
import styles from "./LdCreatorHubDownloadPage.module.css";

type LdChdOsKey = "windows" | "macos";

type LdChdOption = {
  os: string;
  osKey: LdChdOsKey;
  url: string;
  arch: string;
};

// The page's own white glyphs: one source, so the CTA and the "Also available on" icons cannot drift apart.
const OS_GLYPH: Record<LdChdOsKey, string> = {
  windows: ICON.Windows,
  macos: ICON.macOS,
};

function toUiOption(opt: LdChdOption) {
  return { text: opt.os, image: OS_GLYPH[opt.osKey], link: opt.url, arch: opt.arch };
}

type LdCreatorHubDownloadPageProps<O extends LdChdOption> = {
  version?: string;
  /** None when the visitor's system has no installer: the page then shows its title and banner only. */
  primary?: O | null;
  secondary?: O[];
  onDownload?: (option: O, e: MouseEvent<HTMLAnchorElement>) => void;
};

export default function LdCreatorHubDownloadPage<O extends LdChdOption>({
  version = "",
  primary = null,
  secondary = [],
  onDownload = undefined,
}: LdCreatorHubDownloadPageProps<O>) {
  // Each drawn option keeps the caller's option, so a click gives the caller its own object back.
  const source = new Map<object, O>();
  const draw = (option: O) => {
    const ui = toUiOption(option);
    source.set(ui, option);
    return ui;
  };
  return (
    <div
      className={styles.route}
      data-os={primary?.osKey}
      data-arch={primary?.arch}
      data-version={version}
    >
      <StCreatorHubDownload
        primaryOption={primary ? draw(primary) : null}
        secondaryOptions={primary ? secondary.map(draw) : []}
        onDownload={onDownload && ((ui, e) => {
          const option = source.get(ui);
          if (option) onDownload(option, e);
        })}
      />

      {primary && (
        <noscript>
          <a href={primary.url}>Download Decentraland Creator Hub for {primary.os}</a>
        </noscript>
      )}
    </div>
  );
}
