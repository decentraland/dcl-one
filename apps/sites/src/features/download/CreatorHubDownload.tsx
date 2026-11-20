import type { MouseEvent } from "react";

import Tooltip from "@ui/atoms/Tooltip";

import type { DownloadOption } from "@/data/lib/download/creator-hub.server";

import OtherBuilds, { downloadFor, OsIcon } from "./OtherBuilds";
import styles from "./CreatorHubDownload.module.css";

const copy = { download: "Download Creator Hub" };

type Props = {
  primary: DownloadOption;
  others: DownloadOption[];
  onDownload: (option: DownloadOption, kind: "primary" | "secondary", event: MouseEvent<HTMLAnchorElement>) => void;
  className?: string;
};

/** The Creator Hub installer button for this system, named by a tooltip, and the other builds under it. */
export default function CreatorHubDownload({ primary, others, onDownload, className }: Props) {
  return (
    <>
      <Tooltip label={downloadFor(primary)} side="top" className={className}>
        <a
          className={styles.cta}
          href={primary.url}
          data-os={primary.osKey}
          data-arch={primary.arch}
          onClick={(event) => onDownload(primary, "primary", event)}
        >
          <span className={styles.label}>{copy.download}</span>
          <OsIcon os={primary.osKey} className={styles.icon} />
        </a>
      </Tooltip>
      <OtherBuilds
        options={others}
        className={className}
        onDownload={(option, event) => onDownload(option, "secondary", event)}
      />
    </>
  );
}
