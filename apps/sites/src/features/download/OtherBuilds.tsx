import type { MouseEvent } from "react";

import { GeneralApple, GeneralWindows, Tooltip } from "@dcl/one-ui";

import type { DownloadOption } from "@/data/lib/download/creator-hub.server";

import styles from "./OtherBuilds.module.css";

const copy = {
  alsoAvailableOn: "Also available on",
  downloadFor: (build: string) => `Download for ${build}`,
};

export function OsIcon({ os, className }: { os: DownloadOption["osKey"]; className?: string }) {
  const Icon = os === "macos" ? GeneralApple : GeneralWindows;
  return <Icon size={20} className={className} aria-hidden="true" />;
}

/** A Mac build is named by its chip; Windows has one build, and a releases-page placeholder is no build yet. */
const byChip = (option: DownloadOption) => option.osKey === "macos" && Boolean(option.fileName);

export const buildName = (option: DownloadOption) => (byChip(option) ? `${option.os} (${option.archLabel})` : option.os);

export const downloadFor = (option: DownloadOption) => copy.downloadFor(buildName(option));

type Props = {
  options: DownloadOption[];
  onDownload: (option: DownloadOption, event: MouseEvent<HTMLAnchorElement>) => void;
  className?: string;
};

/** The Creator Hub builds offered besides the main button, each named by a tooltip. */
export default function OtherBuilds({ options, onDownload, className }: Props) {
  if (options.length === 0) return null;
  return (
    <div className={className ? `${styles.row} ${className}` : styles.row}>
      {copy.alsoAvailableOn}
      {options.map((option) => (
        <Tooltip key={`${option.osKey}-${option.arch}`} label={downloadFor(option)} side="top" labelChild>
          <a
            className={styles.build}
            href={option.url}
            aria-label={downloadFor(option)}
            data-os={option.osKey}
            data-arch={option.arch}
            onClick={(event) => onDownload(option, event)}
          >
            <OsIcon os={option.osKey} />
            {byChip(option) && <span>{option.archLabel}</span>}
          </a>
        </Tooltip>
      ))}
    </div>
  );
}
