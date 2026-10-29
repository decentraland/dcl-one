import { afterEach, expect, test, vi } from "vitest";
import { nodeMapImage, publicThumbnail, publicThumbnailSet } from "./thumbnail";

afterEach(() => vi.unstubAllEnvs());

test("SSR and hydrated thumbnails share the same public URL even with a different server API origin", () => {
  vi.stubEnv("CATALYST_URL", "https://catalyst.example.org");
  vi.stubEnv("VITE_CATALYST_URL", "");
  const source = "https://worlds-content-server.decentraland.org/contents/bafytest";
  vi.stubEnv("SSR", true);
  const server = publicThumbnail(source);
  vi.stubEnv("SSR", false);
  expect(publicThumbnail(source)).toBe(server);
  expect(server).toBe("/media/convert?width=640&url=" + encodeURIComponent(source));
});

test("an explicit public media origin is shared by both renders", () => {
  vi.stubEnv("VITE_CATALYST_URL", "https://media.example.com/");
  for (const ssr of [true, false]) {
    vi.stubEnv("SSR", ssr);
    expect(publicThumbnail("https://peer.decentraland.org/content/contents/test")).toMatch(/^https:\/\/media.example.com\/media\/convert\?/);
  }
});

test("known public image sources use bounded thumbnails without rewriting private, signed or animated images", () => {
  const source = "https://peer-ec1.decentraland.org/content/contents/bafytest";
  const result = new URL(publicThumbnail(source, 320)!, "https://example.org");
  expect(result.pathname).toBe("/media/convert");
  expect(result.searchParams.get("width")).toBe("320");
  expect(result.searchParams.get("url")).toBe(source);
  expect(publicThumbnail("https://events-assets-099ac00.decentraland.org/poster/poster.webp")).toContain("/media/convert?width=640");
  for (const url of ["/private/image", "data:image/png;base64,AA==", "https://example.com/photo.jpg", `${source}?signature=secret`, "https://events-assets-099ac00.decentraland.org/poster/poster.gif"]) expect(publicThumbnail(url)).toBe(url);
});

test("local catalog thumbnails use native derivatives instead of the remote URL converter", () => {
  vi.stubEnv("VITE_CATALYST_URL", "https://catalyst.example.org");
  const source = "https://catalyst.example.org/lambdas/collections/contents/urn:decentraland:matic:collections-v2:0xabc:1/thumbnail";
  expect(publicThumbnail(source, 80)).toBe(`${source}?width=80`);
  expect(publicThumbnailSet(source, [80, 160])).toBe(`${source}?width=80 80w, ${source}?width=160 160w`);
  vi.stubEnv("VITE_CATALYST_URL", "https://my-node.example/");
  const local = source.replace("catalyst.example.org", "my-node.example");
  expect(publicThumbnail(local, 320)).toBe(`${local}?width=320`);
  expect(publicThumbnail(source, 80)).toBe(source);
});

test("unknown, credentialed and signed sources never get synthetic responsive variants", () => {
  for (const source of [
    "https://unknown.example/lambdas/collections/contents/test/thumbnail",
    "https://peer.decentraland.org/image.png?signature=secret",
    "https://user:password@peer.decentraland.org/image.png",
    "https://peer.decentraland.org/image.gif", "/local.webp",
  ]) {
    expect(publicThumbnail(source, 320)).toBe(source);
    expect(publicThumbnailSet(source, [320, 640])).toBeUndefined();
  }
});

test("the embedded player sends thumbnails through the site it stands in for", () => {
  vi.stubEnv("VITE_CATALYST_URL", "");
  const w = window as Window & { __DCL_PLAY_EMBEDDED__?: boolean; __SITE_BASE__?: string };
  const source = "https://events-assets-099ac00.decentraland.org/poster/poster.webp";
  try {
    w.__DCL_PLAY_EMBEDDED__ = true;
    w.__SITE_BASE__ = "https://example.org/";
    expect(publicThumbnail(source)).toBe("https://example.org/media/convert?width=640&url=" + encodeURIComponent(source));
    delete w.__DCL_PLAY_EMBEDDED__;
    expect(publicThumbnail(source)).toBe("/media/convert?width=640&url=" + encodeURIComponent(source));
  } finally {
    delete w.__DCL_PLAY_EMBEDDED__;
    delete w.__SITE_BASE__;
  }
});


test("production event posters get the same bounded derivatives during SSR and hydration", () => {
  vi.stubEnv("VITE_CATALYST_URL", "");
  for (const folder of ["poster", "poster-vertical"]) {
    const source = `https://events-assets-099ac00.interconnected.online/${folder}/local-upload.webp`;
    for (const ssr of [true, false]) {
      vi.stubEnv("SSR", ssr);
      expect(publicThumbnail(source, 320)).toBe(`/media/convert?width=320&url=${encodeURIComponent(source)}`);
      expect(publicThumbnailSet(source, [320, 640])).toContain("width=640");
    }
  }
  for (const source of [
    "https://events-assets-099ac00.interconnected.online/private/image.webp",
    "https://events-assets-099ac00.interconnected.online/poster/image.gif",
    "https://events-assets-099ac00.interconnected.online/poster/image.webp?signature=secret",
  ]) {
    expect(publicThumbnail(source, 320)).toBe(source);
    expect(publicThumbnailSet(source, [320, 640])).toBeUndefined();
  }
});

test("another node's poster copies are sized from the upstream bucket, its own uploads are not", () => {
  vi.stubEnv("VITE_CATALYST_URL", "");
  for (const host of ["events-assets-099ac00.example.org", "events-assets-099ac00.interconnected.online.example"]) {
    for (const folder of ["poster", "poster-vertical"]) {
      const upstream = `https://events-assets-099ac00.decentraland.org/${folder}/1ad0d5dd6a03ac29.jpg`;
      expect(publicThumbnail(`https://${host}/${folder}/1ad0d5dd6a03ac29.jpg`, 320)).toBe(`/media/convert?width=320&url=${encodeURIComponent(upstream)}`);
    }
    const upload = `https://${host}/poster/${"ab".repeat(32)}.webp`;
    expect(publicThumbnail(upload, 320)).toBe(upload);
    expect(publicThumbnailSet(upload, [320, 640])).toBeUndefined();
    for (const other of [`https://${host}/private/image.webp`, `https://${host}/poster/image.gif`]) expect(publicThumbnail(other, 320)).toBe(other);
  }
});

test("a Foundation peer's collection thumbnail comes from the serving node's own derivative", () => {
  vi.stubEnv("VITE_CATALYST_URL", "");
  const path = "/lambdas/collections/contents/urn:decentraland:matic:collections-v2:0xabc:0/thumbnail";
  for (const host of ["peer.decentraland.org", "peer-ec1.decentraland.org"]) {
    for (const ssr of [true, false]) {
      vi.stubEnv("SSR", ssr);
      expect(publicThumbnail(`https://${host}${path}`, 320)).toBe(`${path}?width=320`);
    }
  }
  expect(publicThumbnailSet(`https://peer.decentraland.org${path}`, [320, 640])).toBe(`${path}?width=320 320w, ${path}?width=640 640w`);
  vi.stubEnv("VITE_CATALYST_URL", "https://media.example.com/");
  expect(publicThumbnail(`https://peer.decentraland.org${path}`, 80)).toBe(`https://media.example.com${path}?width=80`);
});
test("LAND and estate map images come from the serving node's /api mirror", () => {
  vi.stubEnv("VITE_CATALYST_URL", "https://node.example");
  expect(nodeMapImage("https://api.decentraland.org/v1/estates/6513/map.png")).toBe("https://node.example/api/v1/estates/6513/map.png");
  expect(nodeMapImage("https://api.decentraland.org/v2/parcels/-10/42/map.png?size=24")).toBe("https://node.example/api/v2/parcels/-10/42/map.png?size=24");
  expect(nodeMapImage("https://api.decentraland.org/v1/tiles")).toBe("https://api.decentraland.org/v1/tiles");
  expect(nodeMapImage("https://elsewhere.example/v1/estates/1/map.png")).toBe("https://elsewhere.example/v1/estates/1/map.png");
  expect(nodeMapImage(undefined)).toBeUndefined();
});
