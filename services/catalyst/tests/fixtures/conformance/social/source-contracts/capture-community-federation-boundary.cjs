// Node26, actual source server/router/auth/schema/multipart. Local domain
// dependencies; PostgreSQL fault injected only after the auth/schema gates.
const { sourceCapture, SOCIAL_REVISION, sha256, requireDatabaseUrl } = require(
  require("node:path").join(
    require("node:child_process").execFileSync("git", ["-C", __dirname, "rev-parse", "--show-toplevel"], { encoding: "utf8" }).trim(),
    "tooling/social-capture-common.cjs",
  ),
);
const databaseUrl = requireDatabaseUrl(process.argv[4]);
const capture = sourceCapture(process.argv[2]);
const fs = require("fs"),
  net = require("net"),
  root = capture.upstream;
const { createUnsafeIdentity } = require(
  root + "/node_modules/@dcl/crypto/dist/crypto.js",
);
const { Authenticator } = require(root + "/node_modules/@dcl/crypto");
const { createPayload } = require(
  root + "/node_modules/@dcl/crypto-middleware",
);
const { setupHttpRoutes } = require(
  root + "/src/controllers/routes/http.routes.ts",
);
const { createSchemaValidatorComponent } = require(
  root + "/node_modules/@dcl/schema-validator-component",
);
const { createServerComponent } = require(
  root + "/node_modules/@dcl/http-server/dist/server.js",
);
const { getUnderlyingServer } = require(
  root + "/node_modules/@dcl/http-server/dist/injectors.js",
);
const logs = {
    getLogger: () => ({ info() {}, error() {}, warn() {}, debug() {} }),
  },
  id = "00000000-0000-4000-8000-000000000001";
(async () => {
  let server;
  try {
  const pg = new (require(root + "/node_modules/pg").Client)({
    connectionString: databaseUrl,
  });
  await pg.connect();
  let failure;
  try {
  try {
    await pg.query(
      "DO $$ BEGIN RAISE EXCEPTION 'permission denied for table communities'; END $$",
    );
  } catch (error) {
    failure = error;
  }
  } finally { await pg.end(); }
  if (failure?.code !== "P0001") throw Error("missing PG exception");
  let currentInput,
    domainCalls = 0;
  const context = {
    components: {
      logs,
      tracing: { captureException() {} },
      config: { getString: async () => undefined },
      schemaValidator: createSchemaValidatorComponent({
        ensureJsonContentType: false,
      }),
      fetcher: {
        fetch() {
          throw Error("no network");
        },
      },
      communities: {
        updateEditorChoice: async () => {
          domainCalls++;
          if (currentInput.fault) throw failure;
        },
      },
      communityRequests: {
        createCommunityRequest: async () => {
          domainCalls++;
          throw failure;
        },
      },
      communityMembers: { joinCommunity: async () => {} },
    },
  };
  const router = await setupHttpRoutes(context);
  server = await createServerComponent(
    {
      logs,
      config: {
        requireNumber: async () => 0,
        requireString: async () => "127.0.0.1",
      },
    },
    {
      cors: {
        methods: ["GET", "HEAD", "OPTIONS", "DELETE", "POST", "PUT", "PATCH"],
        maxAge: 86400,
      },
      maxBodySize: 2 * 1024 * 1024,
    },
  );
  server.setContext(context);
  server.use(router.middleware());
  server.use(router.allowedMethods());
  await server.start();
  const port = (await getUnderlyingServer(server)).address().port;
  const cases = [];
  for (const input of [
    {
      name: "unsigned-create-no-content-type",
      method: "POST",
      path: "/v1/communities",
    },
    {
      name: "unsigned-create-json",
      method: "POST",
      path: "/v1/communities",
      contentType: "application/json",
    },
    {
      name: "unsigned-patch-json",
      method: "PATCH",
      path: "/v1/communities/" + id,
      contentType: "application/json",
    },
    {
      name: "unsigned-patch-real-database-fault",
      method: "PATCH",
      path: "/v1/communities/" + id,
      contentType: "application/json",
      fault: true,
    },
    {
      name: "unsigned-request-real-database-fault",
      method: "POST",
      path: "/v1/communities/" + id + "/requests",
      contentType: "application/json",
      fault: true,
    },
    {
      name: "signed-patch-real-database-fault",
      method: "PATCH",
      path: "/v1/communities/" + id,
      contentType: "application/json",
      fault: true,
      signed: true,
      sendBody: true,
      length: 22,
    },
    {
      name: "signed-join-json-header-only",
      method: "POST",
      path: "/v1/communities/" + id + "/members",
      contentType: "application/json",
      signed: true,
    },
    {
      name: "signed-create-json-header-only",
      method: "POST",
      path: "/v1/communities",
      contentType: "application/json",
      signed: true,
    },
    ...[524289, 1048577].flatMap((length) =>
      [false, true].map((chunked) => ({
        name: "signed-patch-" + length + (chunked ? "-chunked" : "-declared"),
        method: "PATCH",
        path: "/v1/communities/" + id,
        contentType: "application/json",
        signed: true,
        length,
        chunked,
        sendBody: true,
      })),
    ),
  ]) {
    currentInput = input;
    domainCalls = 0;
    const socket = net.createConnection({ host: "127.0.0.1", port });
    let closed = false,
      received = Buffer.alloc(0),
      resolveResponse,
      rejectResponse,
      response;
    const ready = new Promise((resolve, reject) => {
      resolveResponse = resolve;
      rejectResponse = reject;
    });
    socket.on("close", () => {
      closed = true;
    });
    socket.on("error", (error) => {
      if (!response) rejectResponse(error);
    });
    socket.on("data", (data) => {
      received = Buffer.concat([received, data]);
      if (response) return;
      const end = received.indexOf("\r\n\r\n");
      if (end < 0) return;
      const lines = received.subarray(0, end).toString().split("\r\n");
      const status = Number(lines.shift().split(" ")[1]);
      const headers = {};
      for (const line of lines) {
        const colon = line.indexOf(":");
        headers[line.slice(0, colon).toLowerCase()] = line
          .slice(colon + 1)
          .trim();
      }
      const length = Number(headers["content-length"] || 0);
      if (received.length < end + 4 + length) return;
      response = {
        status,
        headers: Object.fromEntries(
          Object.entries(headers).filter(
            ([key]) =>
              ![
                "date",
                "keep-alive",
                "transfer-encoding",
                "content-length",
              ].includes(key),
          ),
        ),
        body: received.subarray(end + 4, end + 4 + length).toString(),
      };
      resolveResponse(response);
    });
    let head = `${input.method} ${input.path} HTTP/1.1\r\nHost: social.test\r\nConnection: keep-alive\r\nOrigin: https://decentraland.org\r\n`;
    if (input.contentType)
      head += "Content-Type: " + input.contentType + "\r\n";
    if (input.signed) {
      const identity = createUnsafeIdentity(),
        time = Date.now().toString(),
        metadata = "{}",
        payload = createPayload(input.method, input.path, time, metadata);
      for (const [name, value] of Object.entries({
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
      }))
        head += name + ": " + value + "\r\n";
    }
    const length = input.length || 10;
    head += input.chunked
      ? "Transfer-Encoding: chunked\r\n\r\n"
      : `Content-Length: ${length}\r\n\r\n`;
    let body = "";
    if (input.sendBody) {
      const json = JSON.stringify({ editorsChoice: true });
      const data = json + " ".repeat(length - json.length);
      body = input.chunked
        ? `${length.toString(16)}\r\n${data}\r\n0\r\n\r\n`
        : data;
    }
    socket.write(head + body);
    const result = await Promise.race([
      ready,
      new Promise((resolve) =>
        setTimeout(() => resolve({ responseTimeoutAfterMs: 2000 }), 2000),
      ),
    ]);
    await new Promise((resolve) => setTimeout(resolve, 200));
    cases.push({
      ...input,
      ...result,
      domainCalls,
      closedWithinObservation: closed,
    });
    socket.destroy();
  }
  await server.stop();
  server = undefined;
  fs.writeFileSync(
    process.argv[3],
    JSON.stringify(
      {
        source: "decentraland/social-service-ea",
        revision: SOCIAL_REVISION,
        node: process.version,
        cap: 2 * 1024 * 1024,
        observationMs: 200,
        sources: [
          "src/controllers/routes/http.routes.ts",
          "src/controllers/handlers/http/update-community-partially-handler.ts",
          "src/controllers/handlers/http/create-community-request-handler.ts",
          "src/components.ts",
        ].map((path) => ({
          path,
          sha256: sha256(root, path),
        })),
        scope:
          "Actual server/router/auth/schema/multipart middleware. Domain mutators are local stubs, with real PostgreSQL P0001 injected after successful auth/schema for the marked fault case. Header-only cases are observed for 2s; EOF is observed for 200ms after complete response. No inference about longer transport lifetime.",
        cases,
      },
      null,
      2,
    ) + "\n",
  );
  } finally {
    await server?.stop();
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
