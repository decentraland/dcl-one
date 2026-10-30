import { beforeEach, expect, test, vi } from "vitest";
import { sendSignedJSON } from "./client";
import { setPlaceFavorite, setPlaceLike } from "./places";

vi.mock("./client", async (importOriginal) => ({ ...(await importOriginal<typeof import("./client")>()), sendSignedJSON: vi.fn(async () => ({})) }));
const request = vi.mocked(sendSignedJSON);
beforeEach(() => request.mockClear());

test("place likes and favorites send the bodies the node's places API reads", async () => {
  await setPlaceLike("p1", true);
  expect(request).toHaveBeenLastCalledWith("/api/places/p1/likes", { service: "places", method: "PATCH", body: { like: true } });
  await setPlaceLike("p1", null);
  expect(request).toHaveBeenLastCalledWith("/api/places/p1/likes", { service: "places", method: "PATCH", body: { like: null } });
  await setPlaceFavorite("p1", true);
  expect(request).toHaveBeenLastCalledWith("/api/places/p1/favorites", { service: "places", method: "PATCH", body: { favorites: true } });
});
