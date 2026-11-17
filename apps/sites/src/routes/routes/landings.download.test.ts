import { expect, test } from "vitest";
import { loader } from "./landings.download";

test.each([
  ["/landings/download", "/download"],
  ["/landings/download?utm_source=x&next=https%3A%2F%2Fevil.example", "/download?utm_source=x&next=https%3A%2F%2Fevil.example"],
])("%s redirects to %s", (path, location) => {
  const response = loader({ request: new Request(`https://sites.test${path}`) } as never);
  expect(response.status).toBe(308);
  expect(response.headers.get("Location")).toBe(location);
});
