import { expect, it } from "vitest";
import { profileFaceUrl } from "./profile-face-url";

const base = "https://interconnected.online";
const entity = "bafkreihhikmllnupataciq7w4vnznkwbxlyrh773u4ydx7n65mti6brahy";

it.each([
  `https://profile-images.decentraland.org/entities/${entity}/face.png`,
  `https://profile-images-bucket-43d0c58.s3.us-east-1.amazonaws.com/v1/entities/${entity}/face.png`,
])("loads the upstream avatar through the local image service: %s", face => {
  expect(profileFaceUrl(face, base)).toBe(`${base}/profile-images/entities/${entity}/face.png`);
});

it("preserves content snapshots, local images and unrelated origins", () => {
  expect(profileFaceUrl(entity, base)).toBe(`${base}/content/contents/${entity}`);
  for (const face of ["data:image/png;base64,AA", "/profile-images/entities/local/face.png", "https://other.example/face.png", `https://profile-images.decentraland.org.evil.example/entities/${entity}/face.png`]) {
    expect(profileFaceUrl(face, base)).toBe(face);
  }
  expect(profileFaceUrl(undefined, base)).toBe("");
  expect(profileFaceUrl("https://[", base)).toBe("");
});
