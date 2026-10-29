import { describe, expect, it } from "vitest";
import { sameOriginContentUrl } from "./profile";

describe("sameOriginContentUrl", () => {
  it("re-bases upstream profile-image faces onto the local renderer", () => {
    expect(sameOriginContentUrl("https://profile-images.decentraland.org/entities/bafkreiabc123/face.png", { base: "https://x.test" }))
      .toBe("https://x.test/profile-images/entities/bafkreiabc123/face.png");
    expect(sameOriginContentUrl("https://profile-images-bucket-43d0c58.s3.us-east-1.amazonaws.com/v1/entities/bafyq/face.png", { base: "https://x.test" }))
      .toBe("https://x.test/profile-images/entities/bafyq/face.png");
  });
  it("still re-bases catalyst content hashes and leaves other URLs alone", () => {
    expect(sameOriginContentUrl("https://peer.example/content/contents/bafyhash", { base: "https://x.test" }))
      .toBe("https://x.test/content/contents/bafyhash");
    expect(sameOriginContentUrl("https://cdn.example/other.png", { base: "https://x.test" })).toBe("https://cdn.example/other.png");
  });
});
