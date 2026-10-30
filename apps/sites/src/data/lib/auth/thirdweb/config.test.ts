import { expect, it } from "vitest";

import { thirdwebSecretKey } from "./config";

it("uses the thirdweb secret only on a mirror node", () => {
  const secret = { THIRDWEB_SECRET_KEY: "server-only" };
  expect(thirdwebSecretKey({ ...secret, NODE_POSTURE: "mirror" })).toBe("server-only");
  for (const posture of ["island", "journey", "", undefined]) {
    expect(thirdwebSecretKey({ ...secret, NODE_POSTURE: posture })).toBe("");
  }
  expect(thirdwebSecretKey({ NODE_POSTURE: "mirror" })).toBe("");
});
