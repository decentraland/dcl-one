import { catalystBase } from "../catalyst/client";

const PROFILE_IMAGE_HOSTS = new Set([
  "profile-images.decentraland.org",
  "profile-images-bucket-43d0c58.s3.us-east-1.amazonaws.com",
]);

/** Use the local image cache instead of the upstream cookie-setting redirect. */
export function profileFaceUrl(face: string | undefined, base = catalystBase()): string {
  return profileSnapshotUrl(face, "face", base);
}

export function profileBodyUrl(body: string | undefined, base = catalystBase()): string {
  return profileSnapshotUrl(body, "body", base);
}

function profileSnapshotUrl(face: string | undefined, kind: "face" | "body", base: string): string {
  if (!face) return "";
  if (/^https?:\/\//i.test(face)) {
    try {
      const url = new URL(face);
      const path = url.pathname.match(/^\/(?:v1\/)?entities\/([a-zA-Z0-9]+)\/(face|body)\.png$/);
      if (PROFILE_IMAGE_HOSTS.has(url.hostname) && path && path[2] === kind) {
        return `${base}/profile-images/entities/${path[1]}/${kind}.png`;
      }
    } catch {
      return "";
    }
    return face;
  }
  if (face.startsWith("data:") || face.startsWith("/")) return face;
  return `${base}/content/contents/${face}`;
}
