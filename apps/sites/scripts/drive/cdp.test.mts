import assert from "node:assert/strict";
import { test } from "vitest";

import { Tab } from "./cdp.mts";

type Bare = { cmd: (method: string) => Promise<unknown>; ev: (expression?: string) => Promise<unknown>; navigate: Tab["navigate"] };

const bare = (): Bare => Object.create(Tab.prototype) as Bare;

test("navigation does not inspect readiness in a previous document", async () => {
  const calls: string[] = [];
  let frames = 0;
  const tab = bare();
  tab.cmd = async (method: string) => {
    calls.push(method);
    if (method === "Page.navigate") return { frameId: "main", loaderId: "new" };
    return { frameTree: { frame: { id: "main", loaderId: ++frames === 1 ? "old" : "new" } } };
  };
  tab.ev = async () => {
    assert.equal(frames, 2);
    calls.push("evaluate");
    return true;
  };
  await tab.navigate("https://site.example/page", 500);
  // the committed loader, then the load state, the best-effort fonts and images, and a painted frame
  assert.deepEqual(calls, ["Page.navigate", "Page.getFrameTree", "Page.getFrameTree", "evaluate", "evaluate", "evaluate"]);
});

test("same-document navigation uses readiness without waiting for another loader", async () => {
  const tab = bare();
  tab.cmd = async (method: string) => {
    assert.equal(method, "Page.navigate");
    return { frameId: "main" };
  };
  let evaluations = 0;
  tab.ev = async () => { evaluations++; return true; };
  await tab.navigate("https://site.example/page#section", 100);
  assert.equal(evaluations, 3);
});

test("a loader that never commits cannot pass the navigation deadline", async () => {
  const tab = bare();
  tab.cmd = async (method: string) => method === "Page.navigate"
    ? { frameId: "main", loaderId: "new" }
    : { frameTree: { frame: { id: "main", loaderId: "old" } } };
  tab.ev = async () => { assert.fail("readiness ran in the previous document"); };
  await assert.rejects(tab.navigate("https://site.example/page", 5), /page condition did not become true/);
});

test("fonts or images that never settle do not fail a loaded page", async () => {
  const tab = bare();
  tab.cmd = async () => ({ frameId: "main" });
  const asked: string[] = [];
  tab.ev = async (expression?: string) => {
    asked.push(String(expression).slice(0, 20));
    return !String(expression).includes("document.fonts");
  };
  await tab.navigate("https://site.example/page", 20);
  assert.ok(asked.some((a) => a.startsWith("document.fonts")));
  assert.ok(asked.at(-1)?.startsWith("new Promise"));
});
