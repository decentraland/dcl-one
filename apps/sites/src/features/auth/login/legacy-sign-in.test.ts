import { describe, expect, it } from "vitest";

import { blogSignInReturnPath, signInReturnPath } from "./legacy-sign-in";

const at = (url: string) => signInReturnPath(new URL(url));
const blogAt = (url: string) => blogSignInReturnPath(new URL(url));

describe("signInReturnPath (decentraland/sites SignInRedirect + utils/authRedirect)", () => {
  it.each([
    ["the redirectTo path and query", "https://decentraland.org/sign-in?redirectTo=%2Fevents%3Fid%3D1", "/events?id=1"],
    ["only the path and query of a redirectTo on another site", "https://decentraland.org/sign-in?redirectTo=https%3A%2F%2Fevil.example%2Fx%3Fa%3D1%23h", "/x?a=1"],
    ["the URL itself, query included, without a redirectTo", "https://decentraland.org/sign-in?ref=nav", "/sign-in?ref=nav"],
    ["the URL itself when redirectTo is empty", "https://decentraland.org/sign-in?redirectTo=", "/sign-in?redirectTo="],
    ["the URL itself on any host", "http://localhost:5173/sign-in", "/sign-in"],
  ])("should return %s", (_case, url, expected) => {
    expect(at(url)).toBe(expected);
  });

  it("should return the URL itself when the redirectTo cannot be read as a URL", () => {
    expect(at("https://decentraland.org/sign-in?redirectTo=http%3A%2F%2F%5B")).toBe("/sign-in?redirectTo=http%3A%2F%2F%5B");
  });
});

describe("blogSignInReturnPath (decentraland/sites utils/blogAuthRedirect)", () => {
  it.each([
    ["the URL itself under /blog, without a redirectTo", "https://decentraland.org/blog/sign-in", "/blog/sign-in"],
    ["a redirectTo put under /blog", "https://decentraland.org/blog/sign-in?redirectTo=%2Fannouncements", "/blog/announcements"],
    ["a redirectTo already under /blog, not doubled", "https://decentraland.zone/blog/sign-in?redirectTo=%2Fblog%2Fx", "/blog/x"],
    ["the blog root for an empty redirectTo", "https://decentraland.today/blog/sign-in?redirectTo=", "/blog/"],
    ["the path as given on any other host", "http://localhost:5173/blog/sign-in?redirectTo=%2Fevents", "/events"],
    ["the URL itself on any other host", "http://localhost:5173/blog/sign-in", "/blog/sign-in"],
  ])("should return %s", (_case, url, expected) => {
    expect(blogAt(url)).toBe(expected);
  });
});
