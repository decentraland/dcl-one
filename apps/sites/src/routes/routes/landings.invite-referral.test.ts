import { expect, test } from "vitest";
import { loader } from "./landings.invite-referral";

test.each([
  ["/landings/invite-referral?referrer=0xAbC", "/invite/0xAbC"],
  ["/landings/invite-referral?referrer=bevynaut2&step=rewards", "/invite/bevynaut2?step=rewards"],
  ["/landings/invite-referral", "/invite"],
])("%s redirects to %s", (path, location) => {
  const response = loader({ request: new Request(`https://sites.test${path}`) });
  expect(response.status).toBe(308);
  expect(response.headers.get("Location")).toBe(location);
});
