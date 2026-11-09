const replacements: Record<string, readonly (readonly [string | RegExp, string])[]> = {
  "apis/README.md": [
    ["| Asset Bundle Registry | Asset optimization    | Unity asset bundles for optimized loading          |",
      "| Asset Bundle Registry | Asset optimization    | Prebuilt asset bundles for optimized loading       |"],
  ],
  "contributor/architecture/architecture.md": [
    ["**Unity Explorer** - Primary desktop client", "**Desktop Explorer** - Primary desktop client"],
    ["Platform-optimized assets (Unity bundles)", "Platform-optimized asset bundles"],
  ],
  "contributor/architecture/services.md": [
    ["Builds optimized Unity bundles", "Builds optimized asset bundles"],
  ],
  "creator/3d-modeling/3d-models.md": [
    ["### **Export To glTF From Unity**", "### **Export With An Editor Plugin**"],
    [/Unity doesn't support exporting to glTF by default, but you can install a plugin to enable it\.\s+Download the plugin from \[this link\]\(https:\/\/github\.com\/sketchfab\/Unity-glTF-Exporter\)\./g,
      "For editor projects that lack a built-in glTF exporter, check the editor and version requirements of the [Sketchfab exporter](https://github.com/sketchfab/Unity-glTF-Exporter) before installing it."],
  ],
  "creator/3d-modeling/meshes.md": [
    ["Unity's frustum culling", "the renderer's frustum culling"],
  ],
  "creator/build-for-mobile/mobile-client/missing-features.md": [
    ["Decentraland desktop (Unity) client", "Decentraland production desktop client"],
    ["first Unity-parity pass", "first desktop-parity pass"],
    ["**Player movement and collisions vs Unity**", "**Player movement and collisions compared with the production desktop client**"],
    ["Unity players' emotes", "production desktop players' emotes"],
  ],
  "creator/sdk7/2d-ui/ui_text.md": [
    ["The Unity explorer", "The production desktop client"],
    ["the Unity explorer", "the production desktop client"],
  ],
};

/** Apply reviewed wording before the page and its search text are derived. */
export function reviewedDocsProse(source: string, markdown: string): string {
  for (const [before, after] of replacements[source] ?? []) markdown = markdown.replaceAll(before, after);
  return markdown;
}

export function searchableText(md: string): string {
  return md
    .replace(/```[\s\S]*?```|~~~[\s\S]*?~~~/g, " ")
    .replace(/!?(?:\[([^\]]*)\])\([^)]*\)/g, "$1")
    .replace(/<[^>]+>|[`*_>#{}|]/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 5000);
}
