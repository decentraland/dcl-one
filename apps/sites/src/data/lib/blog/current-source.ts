// The BlogSource the blog routes use. Production: unbound until owner decision O1 picks the upstream
// (every read answers a generic 503). Local dev only: the committed fixture, loaded behind
// import.meta.env.DEV so production builds drop it (asserted by a build check).
import { unboundBlogSource, type BlogSource } from "./source";

let devSource: Promise<BlogSource> | undefined;

export function currentBlogSource(): Promise<BlogSource> {
  if (import.meta.env.DEV) {
    devSource ??= import("./fixture-source").then((module) =>
      module.createFixtureBlogSource(),
    );
    return devSource;
  }
  return Promise.resolve(unboundBlogSource);
}
