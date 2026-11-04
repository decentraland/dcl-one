// Actual pinned server and router; local requests only.
const { sourceCapture, SOCIAL_REVISION } = require(
  require("node:path").join(
    require("node:child_process").execFileSync("git", ["-C", __dirname, "rev-parse", "--show-toplevel"], { encoding: "utf8" }).trim(),
    "tooling/social-capture-common.cjs",
  ),
);
const capture = sourceCapture(process.argv[2]);
const fs = require("fs"),
  http = require("http");
const root = capture.upstream;
const { createUnsafeIdentity } = require(
  root + "/node_modules/@dcl/crypto/dist/crypto.js",
);
const { Authenticator } = require(root + "/node_modules/@dcl/crypto");
const { createPayload } = require(
  root + "/node_modules/@dcl/crypto-middleware",
);
const { createSchemaValidatorComponent } = require(
  root + "/node_modules/@dcl/schema-validator-component",
);
const { setupHttpRoutes } = require(
  root + "/src/controllers/routes/http.routes.ts",
);
const { resolveMaxRequestBodyBytes } = require(
  root + "/src/utils/requestBodyLimit.ts",
);
const { createRecordConfigComponent } = require(
  root + "/node_modules/@well-known-components/env-config-provider",
);
const { createServerComponent } = require(
  root + "/node_modules/@dcl/http-server/dist/server.js",
);
const { getUnderlyingServer } = require(
  root + "/node_modules/@dcl/http-server/dist/injectors.js",
);
const { CommunityNotFoundError } = require(
  root + "/src/logic/community/errors.ts",
);
const { createCommunityFieldsValidatorComponent } = require(
  root + "/src/logic/community/fields-validator.ts",
);
(async () => {
  let server, largerServer;
  try {
  const boot = [];
  for (const value of [
    undefined,
    "1",
    "64",
    "1048576",
    "16777216",
    "0",
    "-1",
    "1.5",
    "16777217",
    "NaN",
    "Infinity",
    "64junk",
    "1e3junk",
    "0x100",
    "\ufeff64",
    "",
    " 64.0junk",
    "-Infinity",
  ]) {
    try {
      boot.push({
        input: value === undefined ? null : value,
        result: resolveMaxRequestBodyBytes(
          await createRecordConfigComponent({
            HTTP_MAX_REQUEST_BODY_BYTES: value,
          }).getNumber("HTTP_MAX_REQUEST_BODY_BYTES"),
        ),
      });
    } catch (error) {
      boot.push({ input: value, error: error.message });
    }
  }
  const diagnostics = [];
  const context = {
    components: {
      logs: {
        getLogger: () => ({ info() {}, error() {}, warn() {}, debug() {} }),
      },
      tracing: {
        captureException(error) {
          diagnostics.push({
            message: error.message,
            statusCode: error.statusCode,
            status: error.status,
          });
        },
      },
      communityFieldsValidator: {
        validate: async () => ({ name: "Fixture", description: "Fixture" }),
      },
      communities: {
        createCommunity: async () => ({ id: "fixture-handler-reached" }),
        getCommunitiesPublicInformation: async () => ({
          communities: [],
          total: 0,
        }),
        getCommunities: async () => ({ communities: [], total: 0 }),
        getCommunityPublicInformation: async (id) => {
          throw new CommunityNotFoundError(id);
        },
      },
      config: {
        getString: async (key) =>
          key === "API_ADMIN_TOKEN" ? "fixture-admin" : undefined,
      },
      schemaValidator: createSchemaValidatorComponent({
        ensureJsonContentType: false,
      }),
      fetcher: {
        fetch() {
          throw Error("no network");
        },
      },
    },
  };
  const router = await setupHttpRoutes(context);
  const logger = { info() {}, error() {}, warn() {}, debug() {} };
  server = await createServerComponent(
    {
      config: {
        requireNumber: async () => 0,
        requireString: async () => "127.0.0.1",
      },
      logs: { getLogger: () => logger },
    },
    {
      cors: {
        methods: ["GET", "HEAD", "OPTIONS", "DELETE", "POST", "PUT", "PATCH"],
        maxAge: 86400,
      },
      maxBodySize: resolveMaxRequestBodyBytes(64),
    },
  );
  server.setContext(context);
  server.use(router.middleware());
  server.use(router.allowedMethods());
  await server.start();
  const port = (await getUnderlyingServer(server)).address().port,
    cases = [];
  for (const input of [
    { method: "POST", path: "/v1/communities", length: 65 },
    { method: "POST", path: "/v1/communities", length: 65, sendBody: false },
    { method: "GET", path: "/v1/communities", length: 65 },
    { method: "GET", path: "/unknown", length: 65 },
    { method: "OPTIONS", path: "/unknown", length: 65 },
    { method: "TRACK", path: "/unknown", length: 65 },
    { method: "TRACE", path: "/unknown", length: 65 },
    { method: "PROPFIND", path: "/unknown", length: 65 },
    { method: "GET", path: "/unknown", length: 65, noOrigin: true },
    { method: "OPTIONS", path: "/unknown", length: 65, noOrigin: true },
    { method: "POST", path: "/v1/communities", length: 64 },
    { method: "POST", path: "/v1/communities", chunked: 65 },
    { method: "POST", path: "/v1/communities", chunked: 65, signed: true },
    {
      method: "PATCH",
      path: "/v1/communities/00000000-0000-4000-8000-000000000001",
      chunked: 65,
      signed: true,
    },
    { method: "POST", path: "/v1/mutes", chunked: 65, signed: true },
    { method: "DELETE", path: "/v1/mutes", chunked: 65, signed: true },
    { method: "GET", path: "/unknown", chunked: 65 },
    { method: "OPTIONS", path: "/unknown", chunked: 65 },
    {
      method: "GET",
      path: "/v1/communities/?search=body-boundary-empty-fixture",
      length: 0,
    },
    {
      method: "GET",
      path: "/v1/communities/?search=body-boundary-empty-fixture",
      length: 0,
      signed: true,
    },
    {
      method: "GET",
      path: "/v1/communities/00000000-0000-4000-8000-000000000001/",
      length: 0,
    },
    { method: "GET", path: "/unknown/", length: 0 },
    ...[
      ["PUT", "/v1/communities/00000000-0000-4000-8000-000000000001"],
      [
        "PATCH",
        "/v1/communities/00000000-0000-4000-8000-000000000001/members/0x9222222222222222222222222222222222222222",
      ],
      ["POST", "/v1/communities/00000000-0000-4000-8000-000000000001/places"],
      ["POST", "/v1/communities/00000000-0000-4000-8000-000000000001/posts"],
      ["POST", "/v1/communities/00000000-0000-4000-8000-000000000001/requests"],
      [
        "PATCH",
        "/v1/communities/00000000-0000-4000-8000-000000000001/requests/00000000-0000-4000-8000-000000000002",
      ],
      ["POST", "/v1/referral-progress"],
      ["POST", "/v1/referral-email"],
      ["PATCH", "/v1/social-settings"],
      ["POST", "/v1/friendship-requests"],
      [
        "PATCH",
        "/v1/friendship-requests/0x9222222222222222222222222222222222222222",
      ],
      ["POST", "/v1/blocks"],
      ["POST", "/v1/private-voice-chats"],
      ["PATCH", "/v1/private-voice-chats/00000000-0000-4000-8000-000000000001"],
      ["POST", "/v1/community-voice-chats"],
      [
        "PATCH",
        "/v1/community-voice-chats/00000000-0000-4000-8000-000000000001/speakers/0x9222222222222222222222222222222222222222",
      ],
    ].map(([method, path]) => ({ method, path, chunked: 65, signed: true })),
    {
      method: "POST",
      path: "/v1/members/0x9222222222222222222222222222222222222222/communities",
      chunked: 65,
      bearer: true,
    },
  ]) {
    diagnostics.length = 0;
    const response = await new Promise((resolve, reject) => {
      const headers = {
        origin: "https://decentraland.org",
        connection: "keep-alive",
        "access-control-request-headers": "authorization,x-test",
      };
      if (input.noOrigin) delete headers.origin;
      if (input.bearer) headers.authorization = "Bearer fixture-admin";
      if (input.signed) {
        const identity = createUnsafeIdentity(),
          time = Date.now().toString(),
          metadata = "{}",
          payload = createPayload(
            input.method,
            input.path.split("?")[0],
            time,
            metadata,
          );
        headers["x-identity-timestamp"] = time;
        headers["x-identity-metadata"] = metadata;
        headers["content-type"] =
          input.method === "POST"
            ? "application/x-www-form-urlencoded"
            : "application/json";
        headers["x-identity-auth-chain-0"] = JSON.stringify({
          type: "SIGNER",
          payload: identity.address,
          signature: "",
        });
        headers["x-identity-auth-chain-1"] = JSON.stringify({
          type: "ECDSA_SIGNED_ENTITY",
          payload,
          signature: Authenticator.createSignature(identity, payload),
        });
      }
      if (input.length !== undefined) headers["content-length"] = input.length;
      else headers["transfer-encoding"] = "chunked";
      const request = http.request(
        {
          hostname: "127.0.0.1",
          port,
          path: input.path,
          method: input.method,
          headers,
          agent: false,
        },
        (res) => {
          const parts = [];
          res.on("data", (part) => parts.push(part));
          res.on("end", () =>
            resolve({
              status: res.statusCode,
              body: Buffer.concat(parts).toString(),
              headers: Object.fromEntries(
                Object.entries(res.headers).filter(
                  ([name]) =>
                    !["date", "keep-alive", "transfer-encoding"].includes(name),
                ),
              ),
            }),
          );
        },
      );
      request.on("error", reject);
      request.end(
        input.sendBody === false
          ? undefined
          : Buffer.alloc(input.length ?? input.chunked, 97),
      );
    });
    cases.push({
      ...input,
      ...response,
      diagnostics: JSON.parse(JSON.stringify(diagnostics)),
    });
  }
  await server.stop();
  server = undefined;
  // A larger global cap does not raise the independent native multipart cap
  // or the thumbnail field's byte range. Use parseable multipart, not junk.
  const largerCap = 2 * 1024 * 1024;
  context.components.communityFieldsValidator =
    await createCommunityFieldsValidatorComponent({
      config: { getString: async () => undefined },
    });
  largerServer = await createServerComponent(
    {
      config: {
        requireNumber: async () => 0,
        requireString: async () => "127.0.0.1",
      },
      logs: { getLogger: () => logger },
    },
    {
      cors: {
        methods: ["GET", "HEAD", "OPTIONS", "DELETE", "POST", "PUT", "PATCH"],
        maxAge: 86400,
      },
      maxBodySize: resolveMaxRequestBodyBytes(largerCap),
    },
  );
  largerServer.setContext(context);
  largerServer.use(router.middleware());
  largerServer.use(router.allowedMethods());
  await largerServer.start();
  const largerPort = (await getUnderlyingServer(largerServer)).address().port;
  const multipartCases = [];
  for (const spec of [
    { name: "total-declared", thumbnailBytes: 1024, paddingBytes: 1024 * 1024 },
    {
      name: "total-chunked",
      thumbnailBytes: 1024,
      paddingBytes: 1024 * 1024,
      chunked: true,
    },
    {
      name: "total-header-only",
      thumbnailBytes: 1024,
      paddingBytes: 1024 * 1024,
      sendBody: false,
    },
    {
      name: "thumbnail-over-max",
      thumbnailBytes: 500 * 1024 + 1,
      paddingBytes: 0,
    },
    { name: "thumbnail-under-min", thumbnailBytes: 1023, paddingBytes: 0 },
  ]) {
    const png = Buffer.alloc(spec.thumbnailBytes);
    Buffer.from("89504e470d0a1a0a", "hex").copy(png);
    const form = new FormData();
    form.set("name", "Fixture");
    form.set("description", "Fixture");
    form.set(
      "thumbnail",
      new Blob([png], { type: "image/png" }),
      "fixture.png",
    );
    form.set("unused", "x".repeat(spec.paddingBytes));
    const native = new Request("http://social.test/v1/communities", {
      method: "POST",
      body: form,
    });
    const contentType = native.headers.get("content-type"),
      wire = Buffer.from(await native.arrayBuffer());
    const parsed = await new Response(wire, {
      headers: { "content-type": contentType },
    }).formData();
    const response = await new Promise((resolve, reject) => {
      const identity = createUnsafeIdentity(),
        time = Date.now().toString(),
        metadata = "{}",
        payload = createPayload("POST", "/v1/communities", time, metadata);
      const headers = {
        origin: "https://decentraland.org",
        connection: "keep-alive",
        "content-type": contentType,
        "x-identity-timestamp": time,
        "x-identity-metadata": metadata,
        "x-identity-auth-chain-0": JSON.stringify({
          type: "SIGNER",
          payload: identity.address,
          signature: "",
        }),
        "x-identity-auth-chain-1": JSON.stringify({
          type: "ECDSA_SIGNED_ENTITY",
          payload,
          signature: Authenticator.createSignature(identity, payload),
        }),
      };
      if (spec.chunked) headers["transfer-encoding"] = "chunked";
      else headers["content-length"] = wire.length;
      const request = http.request(
        {
          hostname: "127.0.0.1",
          port: largerPort,
          path: "/v1/communities",
          method: "POST",
          headers,
          agent: false,
        },
        (res) => {
          const chunks = [];
          res.on("data", (chunk) => chunks.push(chunk));
          res.on("end", () =>
            resolve({
              status: res.statusCode,
              body: Buffer.concat(chunks).toString(),
              headers: Object.fromEntries(
                Object.entries(res.headers).filter(
                  ([name]) =>
                    !["date", "keep-alive", "transfer-encoding"].includes(name),
                ),
              ),
            }),
          );
        },
      );
      request.on("error", reject);
      request.end(spec.sendBody === false ? undefined : wire);
    });
    multipartCases.push({
      ...spec,
      wireBytes: wire.length,
      nativeParsedThumbnailBytes: parsed.get("thumbnail").size,
      ...response,
    });
  }
  await largerServer.stop();
  largerServer = undefined;
  fs.writeFileSync(
    process.argv[3],
    JSON.stringify(
      {
        source: "decentraland/social-service-ea",
        revision: SOCIAL_REVISION,
        node: process.version,
        sources: [
          "src/components.ts",
          "src/utils/requestBodyLimit.ts",
          "src/controllers/routes/http.routes.ts",
          "node_modules/@dcl/http-server/dist/server.js",
          "node_modules/@dcl/http-server/dist/logic.js",
          "node_modules/@dcl/schema-validator-component/dist/src/component.js",
          "src/utils/multipart.ts",
          "src/logic/community/fields-validator.ts",
        ],
        scope:
          "Actual server with maxBodySize configured; real route crypto/schema/multipart middleware; mocked domain dependencies are not reached by oversized reads.",
        cap: 64,
        packages: Object.fromEntries(
          [
            "@dcl/http-server",
            "@dcl/schema-validator-component",
            "@well-known-components/env-config-provider",
          ].map((name) => [
            name,
            require(root + "/node_modules/" + name + "/package.json").version,
          ]),
        ),
        boot,
        cases,
        multipart: {
          cap: largerCap,
          bodyCap: 1024 * 1024,
          thumbnailMin: 1024,
          thumbnailMax: 500 * 1024,
          cases: multipartCases,
        },
      },
      null,
      2,
    ) + "\n",
  );
  } finally {
    try { await largerServer?.stop(); } finally { await server?.stop(); }
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
