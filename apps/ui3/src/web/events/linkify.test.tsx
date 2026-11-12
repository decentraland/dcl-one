import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { linkifyText, replaceEmojiShortcodes } from "./linkify";

const html = (text: string) => renderToStaticMarkup(<p>{linkifyText(text)}</p>);

describe("linkifyText", () => {
  it("links bare URLs without their trailing punctuation or unbalanced parenthesis", () => {
    expect(html("See https://example.test/a. (or https://example.test/b)")).toBe(
      '<p>See <a href="https://example.test/a" target="_blank" rel="noopener noreferrer">https://example.test/a</a>. (or <a href="https://example.test/b" target="_blank" rel="noopener noreferrer">https://example.test/b</a>)</p>',
    );
  });
  it("links markdown links by their label and leaves other protocols as text", () => {
    expect(html("[Join :wave:](https://example.test/j) javascript:alert(1)")).toBe(
      '<p><a href="https://example.test/j" target="_blank" rel="noopener noreferrer">Join 👋</a> javascript:alert(1)</p>',
    );
  });
  it("replaces known shortcodes and keeps unknown ones", () => {
    expect(replaceEmojiShortcodes(":tada: party :nope:")).toBe("🎉 party :nope:");
  });
});
