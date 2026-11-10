import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { formatMessage } from "./format";

describe("formatMessage", () => {
  it("fills placeholders and returns plain text when every value is text", () => {
    expect(
      formatMessage("Hi {name}, {count} new", { name: "Ana", count: 3 }),
    ).toBe("Hi Ana, 3 new");
  });

  it("renders tagged text through the caller's handler", () => {
    const html = renderToStaticMarkup(
      <>
        {formatMessage("Stay updated with <b>Decentraland</b>", {
          b: (chunks) => <strong>{chunks}</strong>,
        })}
      </>,
    );
    expect(html).toBe("Stay updated with <strong>Decentraland</strong>");
  });

  it("keeps the text of a tag without a handler and never emits the tag", () => {
    expect(
      formatMessage("For <span>Windows</span> & <span>macOS</span>."),
    ).toBe("For Windows & macOS.");
  });

  it("renders a React value such as a link in place of its placeholder", () => {
    const html = renderToStaticMarkup(
      <>{formatMessage("Retry {link}", { link: <a href="/x">here</a> })}</>,
    );
    expect(html).toBe('Retry <a href="/x">here</a>');
  });

  it("escapes markup that arrives in a value instead of injecting it", () => {
    const html = renderToStaticMarkup(
      <>
        {formatMessage("Hi {name}", { name: "<img src=x onerror=alert(1)>" })}
      </>,
    );
    expect(html).toBe("Hi &lt;img src=x onerror=alert(1)&gt;");
  });

  it("leaves a missing value visible rather than dropping text silently", () => {
    expect(formatMessage("Hi {name}")).toBe("Hi {name}");
  });
});
