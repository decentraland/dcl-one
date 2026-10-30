import { expect, it, vi } from "vitest";
import { SdkProjectConnection } from "./sdk-project";

const composite = (a: string, b: string) => JSON.stringify({ version: 1, components: [{ name: "core-schema::Name", data: { 512: { json: { value: a } }, 513: { json: { value: b } } } }] });
function sharedServer() {
  let content = composite("a", "b"), revision = 1;
  let race: (() => void) | undefined;
  const request = vi.fn<typeof fetch>(async (_url, init) => {
    if (init?.method !== "PUT") return Response.json({ content, revision: String(revision) });
    race?.(); race = undefined;
    const body = JSON.parse(init.body as string);
    if (body.revision !== String(revision)) return new Response("conflict", { status: 409 });
    content = body.content; revision++;
    return Response.json({ content, revision: String(revision) });
  });
  return { request, get content() { return content; }, change(value: string) { content = value; revision++; }, race(action: () => void) { race = action; } };
}

it("two editors merge disjoint saves and retain remote additions on repeated saves", async () => {
  const server = sharedServer();
  const one = new SdkProjectConnection("http://localhost:8000", server.request, "fixture-token");
  const two = new SdkProjectConnection("http://localhost:8000", server.request, "fixture-token");
  await Promise.all([one.read("main.composite"), two.read("main.composite")]);
  await one.saveComposite("main.composite", composite("one", "b"));
  const saved = await two.saveComposite("main.composite", composite("a", "two"));
  expect(JSON.parse(saved.content)).toEqual(JSON.parse(composite("one", "two")));
  expect(saved.merged).toBe(true);
  await two.saveComposite("main.composite", composite("a", "two later"));
  expect(JSON.parse(server.content)).toEqual(JSON.parse(composite("one", "two later")));
  expect(server.request.mock.calls.every(([, init]) => (init?.headers as Record<string, string>).authorization === "Bearer fixture-token")).toBe(true);
});

it("retries an actual compare-and-swap race without discarding the intervening save", async () => {
  const server = sharedServer(), editor = new SdkProjectConnection("http://localhost:8000", server.request);
  await editor.read("main.composite");
  server.race(() => server.change(composite("a", "remote")));
  await editor.saveComposite("main.composite", composite("local", "b"));
  expect(JSON.parse(server.content)).toEqual(JSON.parse(composite("local", "remote")));
});

it("same-field conflicts never write, and opening the shared version allows explicit recovery", async () => {
  const server = sharedServer(), editor = new SdkProjectConnection("http://localhost:8000", server.request);
  await editor.read("main.composite");
  server.change(composite("remote", "b"));
  await expect(editor.saveComposite("main.composite", composite("local", "b"))).rejects.toThrow("same scene fields");
  expect(server.request.mock.calls.filter(([, init]) => init?.method === "PUT")).toHaveLength(0);
  const shared = await editor.sharedComposite("main.composite");
  editor.acknowledgeComposite("main.composite", shared.baseline);
  await editor.saveComposite("main.composite", composite("resolved", "b"));
  expect(JSON.parse(server.content)).toEqual(JSON.parse(composite("resolved", "b")));
});

it("previewing remote changes does not rebase an unsynchronized editor", async () => {
  const server = sharedServer(), editor = new SdkProjectConnection("http://localhost:8000", server.request);
  await editor.read("main.composite");
  server.change(composite("remote", "b"));
  await editor.sharedComposite("main.composite");
  await expect(editor.saveComposite("main.composite", composite("local", "b"))).rejects.toThrow("same scene fields");
});

it("an aborted shared save performs no network request", async () => {
  const server = sharedServer(), editor = new SdkProjectConnection("http://localhost:8000", server.request);
  await expect(editor.saveComposite("main.composite", composite("a", "b"), AbortSignal.abort())).rejects.toThrow();
  expect(server.request).not.toHaveBeenCalled();
});
