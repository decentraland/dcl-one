import realm from "@ui/editor/editor-scene-realm.json";

// The scene editor's own scene as a static realm (apps/ui3/editor-scene): its files are public
// files under /editor-scene/; its about is answered here so the scene urn names this deployment's
// origin, as the Explorer loads a scene's files from the urn's baseUrl as given.
export function loader({ request }: { request: Request }) {
  const origin = new URL(request.url).origin;
  const scenesUrn = realm.configurations.scenesUrn.map((urn) =>
    urn.replace(/([?&]baseUrl=)\//, `$1${origin}/`));
  return Response.json(
    { ...realm, configurations: { ...realm.configurations, scenesUrn } },
    { headers: { "Cache-Control": "no-cache" } },
  );
}
