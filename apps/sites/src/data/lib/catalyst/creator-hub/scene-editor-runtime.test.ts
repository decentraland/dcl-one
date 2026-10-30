import { expect, test } from "vitest";
import { editorBusChannel, editorBusChannelFromViewportSrc } from "@ui/editor/editor-bus";
import { buildViewportUrl } from "./scene-editor";

test.each([undefined, "/play", "/play/", "/_play", "/_play/"])(
  "editor and preview use Play's engine URL and cache keys (%s)", playUrl => {
    const launch = { playUrl, realm: "/_project/sessions/00000000-0000-4000-8000-000000000001", position: "2,3", preview: true };
    const editor = new URL(buildViewportUrl({ ...launch, systemScene: "/editor-scene/EditorScene", editorSession: "session" }), "https://sites.test");
    const preview = new URL(buildViewportUrl(launch), "https://sites.test");
    const play = new URL("/play/", "https://sites.test");
    expect(editor.pathname).toBe(play.pathname);
    expect(preview.pathname).toBe(play.pathname);
    // only parameters the Explorer knows; the session rides in the fragment the app reads
    expect([...editor.searchParams.keys()].sort()).toEqual(["position", "preview", "realm", "systemScene"]);
    expect(editor.searchParams.get("systemScene")).toBe("/editor-scene/EditorScene");
    expect(editor.hash).toBe("#editorSession=session");
    expect(preview.searchParams.has("systemScene")).toBe(false);
    expect(preview.hash).toBe("");
    expect(editor.searchParams.get("realm")).toBe(preview.searchParams.get("realm"));
    for (const artifact of ["pkg/manifest.json", "pkg/webgpu_build.js", "engine_worker.js", "service_worker.js",
      "v/1234567890abcdef/pkg/webgpu_build_bg.wasm", "v/1234567890abcdef/assets_bundle.bin",
      "v/1234567890abcdef/assets_animations.bin"]) {
      expect(new URL(artifact, editor).href).toBe(new URL(artifact, play).href);
      expect(new URL(artifact, preview).href).toBe(new URL(artifact, play).href);
    }
  },
);

test.each(["/custom-engine", "https://engine.test/play", "https://engine.test/_play"])(
  "an explicit custom engine origin remains supported (%s)", playUrl => {
    const editor = buildViewportUrl({ playUrl });
    expect(editor.startsWith(`${playUrl}/?`)).toBe(true);
  },
);

test("without an editor scene the viewport is the engine alone, signed in as a guest", () => {
  const session = "6f1c0e52-3c6e-4f53-9a43-2b7b9f8f6d10";
  const editor = new URL(buildViewportUrl({ realm: "/_project", preview: true, editorSession: session, engineOnly: true }), "https://sites.test");
  expect([...editor.searchParams.keys()].sort()).toEqual(["guest", "hud", "position", "preview", "realm"]);
  expect(editor.searchParams.get("hud")).toBe("0");
  expect(editor.searchParams.get("guest")).toBe("1");
  expect(editorBusChannelFromViewportSrc(editor.href)).toBe(editorBusChannel(session));
  const preview = new URL(buildViewportUrl({ realm: "/_project", preview: true }), "https://sites.test");
  expect(preview.searchParams.has("hud")).toBe(false);
  expect(preview.searchParams.has("guest")).toBe(false);
});
