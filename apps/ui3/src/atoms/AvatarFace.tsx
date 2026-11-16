// The Person path: @mui/icons-material 5.18.0 (MIT). The licence text is in ./assets/mui-icons-material/LICENSE.
import { useState } from "react";
import styles from "./AvatarFace.module.css";

// The part of a profile that the face needs.
export type AvatarFaceProfile = {
  name?: string;
  avatar?: { snapshots?: { face256?: string } };
};

export type AvatarFaceProps = {
  avatar?: AvatarFaceProfile | null;
  size?: "tiny" | "small" | "medium" | "large";
  inline?: boolean;
  // Defaults to the profile name. Pass "" when the name is next to the face.
  alt?: string;
  // Prefix for a snapshot that is a content hash and not a URL.
  contentUrl?: string;
  className?: string;
};

const CONTENT_URL = "https://peer.decentraland.org/content/contents/";

export default function AvatarFace({
  avatar,
  size = "small",
  inline = false,
  alt,
  contentUrl = CONTENT_URL,
  className,
}: AvatarFaceProps) {
  const [failed, setFailed] = useState<string | null>(null);
  const face = avatar?.avatar?.snapshots?.face256;
  const src = face && !face.startsWith("https://") ? contentUrl + face : face;
  const name = alt ?? avatar?.name ?? "";
  const showImage = Boolean(src) && failed !== src;

  return (
    <span
      className={className ? styles.root + " " + className : styles.root}
      data-size={size}
      data-inline={inline ? "" : undefined}
    >
      {showImage ? (
        <span className={styles.face}>
          <img className={styles.img} src={src} alt={name} onError={() => setFailed(src ?? null)} />
        </span>
      ) : (
        <span
          className={styles.face}
          role={name ? "img" : undefined}
          aria-label={name || undefined}
          aria-hidden={name ? undefined : true}
        >
          <svg className={styles.glyph} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            <path d="M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4m0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4" />
          </svg>
        </span>
      )}
    </span>
  );
}
