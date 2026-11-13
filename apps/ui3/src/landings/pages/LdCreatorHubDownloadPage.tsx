import type { MouseEvent } from "react";

import StCreatorHubDownload from "../../web/pages/StCreatorHubDownload";
import styles from "./LdCreatorHubDownloadPage.module.css";

type LdChdOsKey = "windows" | "macos";

type LdChdOption = {
  os: string;
  osKey: LdChdOsKey;
  url: string;
  arch: string;
};

const OS_GLYPH: Record<LdChdOsKey, string> = {
  windows:
    "data:image/svg+xml;utf8," +
    encodeURIComponent(
      '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="%23fff" d="M3 5.6 10.3 4.6V11.4H3V5.6Zm0 12.8 7.3 1V12.6H3v5.8Zm8.4 1.2L21 21V12.6h-9.6v8Zm0-15.6V11.4H21V3l-9.6 1Z"/></svg>',
    ),
  macos:
    "data:image/svg+xml;utf8," +
    encodeURIComponent(
      '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="%23fff" d="M16.4 12.6c0-2.3 1.9-3.4 2-3.5-1.1-1.6-2.8-1.8-3.4-1.8-1.4-.1-2.8.8-3.5.8-.7 0-1.8-.8-3-.8-1.5 0-2.9.9-3.7 2.3-1.6 2.8-.4 6.8 1.1 9 .7 1.1 1.6 2.3 2.8 2.3 1.1 0 1.5-.7 2.9-.7 1.3 0 1.7.7 2.9.7 1.2 0 1.9-1.1 2.7-2.2.8-1.3 1.2-2.5 1.2-2.6-.1 0-2.3-.9-2.3-3.5Zm-2.3-6.4c.6-.8 1-1.8.9-2.9-.9 0-2 .6-2.6 1.4-.6.7-1.1 1.7-.9 2.8 1 0 2-.5 2.6-1.3Z"/></svg>',
    ),
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
  const options = primary ? [primary, ...secondary] : [];
  return (
    <div
      className={styles.route}
      data-os={primary?.osKey}
      data-arch={primary?.arch}
      data-version={version}
    >
      <StCreatorHubDownload
        primaryOption={primary ? toUiOption(primary) : null}
        secondaryOptions={primary ? secondary.map(toUiOption) : []}
        onDownload={onDownload && ((ui, e) => {
          const option = options.find((o) => o.url === ui.link && o.os === ui.text);
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
