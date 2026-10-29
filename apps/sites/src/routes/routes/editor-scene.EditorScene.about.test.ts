import { expect, it } from "vitest";

import { loader } from "./editor-scene.EditorScene.about";

it("names the requesting deployment's origin in the editor scene's urn", async () => {
  const about = await loader({ request: new Request("https://sites.test/editor-scene/EditorScene/about") }).json();
  expect(about.configurations.realmName).toBe("EditorScene");
  expect(about.configurations.scenesUrn).toHaveLength(1);
  expect(about.configurations.scenesUrn[0]).toMatch(
    /^urn:decentraland:entity:baf[a-z2-7]+\?=&baseUrl=https:\/\/sites\.test\/editor-scene\/$/,
  );
});
