// @vitest-environment jsdom
import { act, render } from "@testing-library/react";
import { hydrateRoot } from "react-dom/client";
import { renderToString } from "react-dom/server";
import { createRoutesStub } from "react-router";
import { describe, expect, it, vi } from "vitest";

import { Layout, RouteHeadScripts } from "./root";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});

// The literal text, not the exported constant: a change to the script that runs before hydration must fail here.
const EMBED_BOOT = '(function(){try{if(self!==top)document.documentElement.setAttribute("data-embed","1")}catch(e){}})()';

describe("root document head", () => {
  it("runs the embed boot script inline in <head>, before the page renders", () => {
    const Stub = createRoutesStub([{ id: "root", path: "/", Component: () => <Layout>{null}</Layout> }]);
    const html = renderToString(<Stub initialEntries={["/"]} />);
    const head = html.slice(html.indexOf("<head>"), html.indexOf("</head>"));
    expect(head).toContain(`<script>${EMBED_BOOT}</script>`);
  });

  it("runs a route's head script inline in <head> of the document the server sends", () => {
    const Stub = createRoutesStub([{ id: "root", path: "/", Component: () => <Layout>{null}</Layout>, handle: { headScript: "/*cart*/" } }]);
    const html = renderToString(<Stub initialEntries={["/"]} />);
    const head = html.slice(html.indexOf("<head>"), html.indexOf("</head>"));
    expect(head).toContain("<script>/*cart*/</script>");
  });

  it("keeps a route's head script through hydration, and no client render draws it again", async () => {
    const Stub = createRoutesStub([{ id: "root", path: "/", Component: () => <div><RouteHeadScripts /></div>, handle: { headScript: "/*cart*/" } }]);
    const container = document.createElement("div");
    container.innerHTML = renderToString(<Stub initialEntries={["/"]} />);
    expect(container.querySelector("script")?.textContent).toBe("/*cart*/");
    const onRecoverableError = vi.fn();
    await act(async () => void hydrateRoot(container, <Stub initialEntries={["/"]} />, { onRecoverableError }));
    expect(onRecoverableError).not.toHaveBeenCalled();
    expect(container.querySelector("script")).toBeNull();
    const client = render(<Stub initialEntries={["/"]} />);
    expect(client.container.querySelector("script")).toBeNull();
  });
});
