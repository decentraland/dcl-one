import { catalystBase } from "../catalyst/client";

const PROFILE_IMAGE_HOST = /^(?:profile-images\.decentraland\.org|profile-images-bucket-[a-z0-9]+\.s3\.[a-z0-9-]+\.amazonaws\.com)$/;

/**
 * A profile snapshot (`face.png` or `body.png`) through the local image cache, instead of the
 * upstream cookie-setting redirect; a bare content hash resolves to the node's content store.
 */
export function profileSnapshotUrl(
  value: string | undefined,
  file: "face.png" | "body.png",
  base = catalystBase(),
): string {
  if (!value) return "";
  if (/^https?:\/\//i.test(value)) {
    try {
      const url = new URL(value);
      const path = url.pathname.match(/^\/(?:v1\/)?entities\/([a-zA-Z0-9]+)\/(face|body)\.png$/);
      if (PROFILE_IMAGE_HOST.test(url.hostname)) {
        return path && `${path[2]}.png` === file
          ? `${base}/profile-images/entities/${path[1]}/${file}`
          : "";
      }
    } catch {
      return "";
    }
    return value;
  }
  if (value.startsWith("data:") || value.startsWith("/")) return value;
  return `${base}/content/contents/${value}`;
}

/** Use the local image cache instead of the upstream cookie-setting redirect. */
export function profileFaceUrl(face: string | undefined, base = catalystBase()): string {
  return profileSnapshotUrl(face, "face.png", base);
}
