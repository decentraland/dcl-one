import { describe, expect, it } from "vitest";

import { loader as blogSignIn } from "./blog.sign-in";
import { loader as signIn } from "./sign-in";

/** The redirect a loader throws, or a failure when it returns instead. */
function redirectOf(run: () => unknown): Response {
  try {
    run();
  } catch (thrown) {
    if (thrown instanceof Response) return thrown;
    throw thrown;
  }
  throw new Error("the loader did not redirect");
}

const request = (url: string) => ({ request: new Request(url) }) as never;

describe("/sign-in", () => {
  it("should send the visitor to the sign-in page, coming back to redirectTo", () => {
    const response = redirectOf(() => signIn(request("https://decentraland.org/sign-in?redirectTo=%2Fevents")));
    expect(response.status).toBe(302);
    expect(response.headers.get("Location")).toBe("/auth/login?redirectTo=%2Fevents");
  });

  it("should come back to /sign-in itself without a redirectTo, as decentraland.org did", () => {
    const response = redirectOf(() => signIn(request("https://decentraland.org/sign-in")));
    expect(response.headers.get("Location")).toBe("/auth/login?redirectTo=%2Fsign-in");
  });
});

describe("/blog/sign-in", () => {
  it("should send the visitor to the sign-in page with the blog's return path", () => {
    const response = redirectOf(() => blogSignIn(request("https://decentraland.org/blog/sign-in?redirectTo=%2Fannouncements")));
    expect(response.status).toBe(302);
    expect(response.headers.get("Location")).toBe("/auth/login?redirectTo=%2Fblog%2Fannouncements");
  });
});
