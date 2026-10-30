// Docs images are one WebP each (content/docs/_media.json, scripts/docs-images.mts): <img>
// tags whose source is listed get their intrinsic width/height so the layout does not shift.
export interface DocsMediaEntry { source: string; width: number; height: number; bytes: number; animated?: true }

const IMAGE_TAG = /<img\b(?:[^>"']|"[^"]*"|'[^']*')*>/gi;

function attribute(tag: string, name: string): string | undefined {
  const value = tag.match(new RegExp(`\\s${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)'|([^\\s>]+))`, "i"));
  return value ? (value[1] ?? value[2] ?? value[3]).replace(/&(?:amp|quot|lt|gt|#39);/g,
    (entity) => ({ "&amp;": "&", "&quot;": '"', "&lt;": "<", "&gt;": ">", "&#39;": "'" })[entity]!) : undefined;
}

function setAttribute(tag: string, name: string, value: string): string {
  return tag.replace(/\s*\/?\s*>$/, ` ${name}="${value}">`);
}

export function addImageDimensions(html: string, lookup: (src: string) => DocsMediaEntry | undefined): string {
  return html.replace(IMAGE_TAG, (tag) => {
    const src = attribute(tag, "src");
    const image = src ? lookup(src) : undefined;
    if (!image || !(image.width > 0) || !(image.height > 0)) return tag;
    // A percentage or other non-pixel size keeps the author's markup as is.
    if ([attribute(tag, "width"), attribute(tag, "height")].some((v) => v !== undefined && !/^\d+$/.test(v.trim()))) return tag;
    const specifiedWidth = Number(attribute(tag, "width"));
    const specifiedHeight = Number(attribute(tag, "height"));
    const width = specifiedWidth > 0 ? specifiedWidth
      : specifiedHeight > 0 ? Math.round(specifiedHeight * image.width / image.height) : image.width;
    const height = specifiedHeight > 0 ? specifiedHeight : Math.round(width * image.height / image.width);
    let result = tag;
    if (attribute(tag, "width") === undefined) result = setAttribute(result, "width", String(width));
    if (attribute(tag, "height") === undefined) result = setAttribute(result, "height", String(height));
    return result;
  });
}
