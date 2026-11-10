import { describe, expect, it } from "vitest";

import { blogSignInTarget } from "./sign-in";

describe("blogSignInTarget", () => {
  it("should default to the blog home", () => {
    expect(blogSignInTarget(null)).toBe("/blog");
  });

  it("should return to a same-site path, with its query and hash", () => {
    expect(blogSignInTarget("/blog/announcements/a-post?utm=x#h2")).toBe(
      "/blog/announcements/a-post?utm=x#h2",
    );
  });

  it.each([
    "https://evil.example/blog",
    "//evil.example",
    "/\\evil.example",
    "/%2F%2Fevil.example",
    "/%5Cevil.example",
    " /blog",
    "javascript:alert(1)",
    "data:text/html,x",
  ])("should refuse %j and use the blog home", (target) => {
    expect(blogSignInTarget(target)).toBe("/blog");
  });

  it.each([
    "/blog/sign-in",
    "/blog/sign-in?redirectTo=%2Fblog%2Fsign-in",
    "/blog/sign-in#x",
    "/blog/sign-in/",
    "/Blog/Sign-In",
    "/blog/sign%2Din",
  ])("should not send %j back to sign-in itself", (target) => {
    expect(blogSignInTarget(target)).toBe("/blog");
  });
});
