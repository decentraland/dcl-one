/**
 * The Sites Segment transport the browser is handed: the public write key and the optional
 * first-party proxy host (config, not secrets: both ship to every page). Unset, Sites telemetry is
 * a no-op. The root loader and `/internal/session` both read it here, so they agree.
 */
export function sitesSegmentPublicConfig(): { segmentWriteKey: string; segmentApiHost: string } {
  return {
    segmentWriteKey: process.env.SEGMENT_WRITE_KEY || process.env.VITE_SEGMENT_WRITE_KEY || "",
    segmentApiHost: process.env.SEGMENT_API_HOST || process.env.VITE_SEGMENT_API_HOST || "",
  };
}
