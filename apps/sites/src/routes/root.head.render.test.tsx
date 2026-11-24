import { renderToString } from "react-dom/server";
import { createRoutesStub } from "react-router";
import { describe, expect, it } from "vitest";

import { Layout } from "./root";

// The literal text, not the exported constant: a change to the script that runs before hydration must fail here.
const EMBED_BOOT = '(function(){try{if(self!==top)document.documentElement.setAttribute("data-embed","1")}catch(e){}})()';

describe("root document head", () => {
  it("runs the embed boot script inline in <head>, before the page renders", () => {
    const Stub = createRoutesStub([{ id: "root", path: "/", Component: () => <Layout>{null}</Layout> }]);
    const html = renderToString(<Stub initialEntries={["/"]} />);
    const head = html.slice(html.indexOf("<head>"), html.indexOf("</head>"));
    expect(head).toContain(`<script>${EMBED_BOOT}</script>`);
  });

  describe("when the root loader read the node's network", () => {
    it("should publish its network id before any module reads the page's environment", () => {
      const Stub = createRoutesStub([{ id: "root", path: "/", Component: () => <Layout>{null}</Layout> }]);
      const html = renderToString(
        <Stub initialEntries={["/"]} hydrationData={{ loaderData: { root: { nodeNetworkId: 11155111 } } }} />,
      );
      const head = html.slice(html.indexOf("<head>"), html.indexOf("</head>"));
      expect(head).toMatch(/window\.__DCL_PUBLIC__=\{[^<]*(&quot;|")nodeNetworkId(&quot;|"):11155111[,}]/);
    });
  });
});
