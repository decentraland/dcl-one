import { describe, expect, it } from "vitest";

import {
  LEGACY_SMART_ITEM_PREFIX,
  renameLegacySmartItemComponents,
  SMART_ITEM_PREFIX,
  smartItemComponentName,
} from "./smart-item-components";

describe("smartItemComponentName", () => {
  it("reads a legacy-namespaced smart-item component under the current namespace", () => {
    expect(smartItemComponentName(`${LEGACY_SMART_ITEM_PREFIX}Actions`)).toBe(`${SMART_ITEM_PREFIX}Actions`);
    expect(smartItemComponentName("asset-packs::Triggers")).toBe("smart-items::Triggers");
  });

  it("leaves current, core and inspector names alone", () => {
    for (const name of ["smart-items::States", "core::Transform", "core-schema::Name", "inspector::Nodes", "Actions"]) {
      expect(smartItemComponentName(name)).toBe(name);
    }
  });
});

describe("renameLegacySmartItemComponents", () => {
  it("renames every legacy block of a composite and keeps their data and schemas", () => {
    const schema = { type: "object", serializationType: "map" };
    const blocks = renameLegacySmartItemComponents([
      { name: "core::Transform", data: { "512": { json: { parent: 0 } } } },
      { name: "asset-packs::Actions", jsonSchema: schema, data: { "512": { json: { id: 512, value: [] } } } },
      { name: "smart-items::Counter", data: { "512": { json: { id: 512, value: 3 } } } },
    ]);
    expect(blocks.map((b) => b.name)).toEqual(["core::Transform", "smart-items::Actions", "smart-items::Counter"]);
    expect(blocks[1]).toEqual({ name: "smart-items::Actions", jsonSchema: schema, data: { "512": { json: { id: 512, value: [] } } } });
  });

  it("passes blocks that are not objects with a string name through unchanged", () => {
    const junk = [null, 42, "asset-packs::Actions", { data: {} }, { name: 7 }];
    expect(renameLegacySmartItemComponents(junk)).toEqual(junk);
  });
});
