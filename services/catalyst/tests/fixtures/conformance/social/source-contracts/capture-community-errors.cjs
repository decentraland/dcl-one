// Run with Node26 and a checkout at the pinned social-service revision.
// Usage: node capture-community-errors.cjs <source-root> <output> <postgres-url>
const fs = require("fs"),
  crypto = require("crypto"),
  root = process.argv[2];
require(root + "/node_modules/ts-node/register/transpile-only");
const handlers = require(root + "/src/controllers/handlers/http");
const routes = fs.readFileSync(
  root + "/src/controllers/routes/http.routes.ts",
  "utf8",
);
const operations = [
  { method: "GET", path: "/v1/communities/:address/managed" },
  { method: "GET", path: "/v1/communities/:id" },
  { method: "GET", path: "/v1/communities" },
  { method: "GET", path: "/v1/communities/:id/members" },
  { method: "POST", path: "/v1/communities/:id/members" },
  { method: "DELETE", path: "/v1/communities/:id/members/:memberAddress" },
  { method: "PATCH", path: "/v1/communities/:id/members/:address" },
  { method: "GET", path: "/v1/communities/:id/bans" },
  { method: "POST", path: "/v1/communities/:id/members/:memberAddress/bans" },
  { method: "DELETE", path: "/v1/communities/:id/members/:memberAddress/bans" },
  { method: "GET", path: "/v1/members/:address/communities" },
  { method: "POST", path: "/v1/members/:address/communities" },
  { method: "GET", path: "/v1/members/:address/requests" },
  { method: "GET", path: "/v1/communities/:id/requests" },
  { method: "GET", path: "/v1/members/:address/invites" },
  { method: "POST", path: "/v1/communities" },
  { method: "PUT", path: "/v1/communities/:id" },
  { method: "PATCH", path: "/v1/communities/:id" },
  { method: "DELETE", path: "/v1/communities/:id" },
  { method: "GET", path: "/v1/communities/:id/places" },
  { method: "POST", path: "/v1/communities/:id/places" },
  { method: "DELETE", path: "/v1/communities/:id/places/:placeId" },
  { method: "GET", path: "/v1/communities/:id/posts" },
  { method: "POST", path: "/v1/communities/:id/posts" },
  { method: "DELETE", path: "/v1/communities/:id/posts/:postId" },
  { method: "POST", path: "/v1/communities/:id/posts/:postId/like" },
  { method: "DELETE", path: "/v1/communities/:id/posts/:postId/like" },
  { method: "POST", path: "/v1/communities/:id/requests" },
  { method: "PATCH", path: "/v1/communities/:id/requests/:requestId" },
  { method: "GET", path: "/v1/moderation/communities" },
  { method: "GET", path: "/v2/communities/:id" },
  { method: "GET", path: "/v2/communities" },
  { method: "GET", path: "/v2/communities/:id/members" },
  { method: "GET", path: "/v2/communities/:id/bans" },
  { method: "GET", path: "/v2/members/:address/requests" },
  { method: "GET", path: "/v2/communities/:id/requests" },
  { method: "GET", path: "/v2/communities/:id/posts" },
];
const actor = "0x1111111111111111111111111111111111111111",
  target = "0x2222222222222222222222222222222222222222",
  id = "00000000-0000-4000-8000-000000000001";
const log = { info() {}, debug() {}, error() {}, warn() {} };
(async () => {
  const pg = new (require(root + "/node_modules/pg").Client)({
    connectionString: process.argv[4],
  });
  await pg.connect();
  let failure;
  try {
    await pg.query(
      "DO $$ BEGIN RAISE EXCEPTION 'permission denied for table communities'; END $$",
    );
  } catch (error) {
    failure = error;
  }
  await pg.end();
  if (!failure || failure.code !== "P0001")
    throw Error("real PostgreSQL exception was not observed");
  const cases = [];
  const sources = new Set([
    "src/controllers/routes/http.routes.ts",
    "src/utils/errors.ts",
  ]);
  for (const operation of operations) {
    const match = new RegExp(
      "router\\." +
        operation.method.toLowerCase() +
        "\\(\\s*'" +
        operation.path +
        "'",
    ).exec(routes);
    const offset = match?.index;
    if (offset === undefined)
      throw Error("route not located " + JSON.stringify(operation));
    const block = routes.slice(
      offset,
      routes.indexOf("router.", offset + 7) < 0
        ? routes.length
        : routes.indexOf("router.", offset + 7),
    );
    const names = Object.keys(handlers).filter((name) =>
      new RegExp("\\b" + name + "\\b").test(block),
    );
    if (names.length !== 1)
      throw Error(
        "ambiguous handler " + JSON.stringify({ operation, names, block }),
      );
    const handler = names[0],
      trace = [];
    const component = (name) =>
      new Proxy(
        {},
        {
          get: (_, method) => async () => {
            trace.push(name + "." + String(method));
            throw failure;
          },
        },
      );
    const fixed = {
      logs: { getLogger: () => log },
      config: { getString: async () => "fixture-admin" },
      featureFlags: { getVariants: async () => [actor] },
      communityFieldsValidator: {
        validate: async () => ({
          name: "Fixture community",
          description: "fixture description",
          privacy: "public",
          visibility: "all",
          placeIds: [],
        }),
      },
    };
    const components = new Proxy(fixed, {
      get: (value, key) => (key in value ? value[key] : component(String(key))),
    });
    const body = {
      communityIds: [id],
      content: "Fixture post",
      editorsChoice: true,
      role: "member",
      placeIds: [id],
      type: "request_to_join",
      targetedAddress: target,
      intention: "accept",
    };
    const request = new Request(
      "http://social.test" + operation.path.replace(/:[^/]+/g, id),
      {
        method: "POST",
        body: JSON.stringify(body),
        headers: { "content-type": "application/json" },
      },
    );
    const context = {
      components,
      url: new URL("http://social.test/?inviter=" + actor),
      params: {
        id,
        address: operation.path.endsWith("/invites") ? target : actor,
        memberAddress: target,
        placeId: id,
        postId: id,
        requestId: id,
      },
      verification: { auth: actor },
      request,
      formData: {
        fields: {
          name: "Fixture community",
          description: "fixture description",
        },
        files: {},
      },
    };
    let response;
    try {
      response = await handlers[handler](context);
    } catch (error) {
      response = {
        thrown: { name: error.constructor.name, message: error.message },
      };
    }
    if (
      response.status !== 500 ||
      response.body?.message !== failure.message ||
      trace.length !== 1
    )
      throw Error(
        "unexpected boundary " +
          JSON.stringify({ operation, handler, trace, response }),
      );
    const candidates = fs
      .readdirSync(root + "/src/controllers/handlers/http")
      .filter(
        (file) =>
          file.endsWith(".ts") &&
          fs
            .readFileSync(
              root + "/src/controllers/handlers/http/" + file,
              "utf8",
            )
            .includes("function " + handler + "("),
      );
    if (candidates.length !== 1)
      throw Error("handler source not unique " + handler);
    sources.add("src/controllers/handlers/http/" + candidates[0]);
    cases.push({ ...operation, handler, dependency: trace[0], response });
  }
  fs.writeFileSync(
    process.argv[3],
    JSON.stringify(
      {
        baseline: {
          repository: "decentraland/social-service-ea",
          revision: "b6b2cefefdf727b2ed47ddb887e77d226a41eea6",
          sources: [...sources].sort().map((path) => ({
            path,
            sha256: crypto
              .createHash("sha256")
              .update(fs.readFileSync(root + "/" + path))
              .digest("hex"),
          })),
        },
        runtime: { node: process.version },
        scope:
          "All37 actual community handlers catch a real node-pg P0001 exception injected at their domain dependency. Authentication, schemas and domain operations are not executed by this capture. Target replays real SQL permission errors through each route; other driver and operating-system errors are not established.",
        cases,
      },
      null,
      2,
    ) + "\n",
  );
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
