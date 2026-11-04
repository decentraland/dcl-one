// Node26. Actual source handlers/components/SQL adapter over a disposable PG schema.
// Usage: node <script> <source-root> <output> <postgres-url>; no remote providers.
const { sourceCapture, SOCIAL_REVISION, sha256, requireDatabaseUrl } = require(
  require("node:path").join(
    require("node:child_process").execFileSync("git", ["-C", __dirname, "rev-parse", "--show-toplevel"], { encoding: "utf8" }).trim(),
    "tooling/social-capture-common.cjs",
  ),
);
const databaseUrl = requireDatabaseUrl(process.argv[4]);
const capture = sourceCapture(process.argv[2]);
const fs = require("fs"),
  crypto = require("crypto"),
  root = capture.upstream;
const { Client } = require(root + "/node_modules/pg");
const { createCommunitiesDBComponent } = require(
  root + "/src/adapters/communities-db.ts",
);
const { createCommunityComponent } = require(
  root + "/src/logic/community/communities.ts",
);
const { createCommunityRequestsComponent } = require(
  root + "/src/logic/community/requests.ts",
);
const { getMemberRequestsHandler, getMemberRequestsV2Handler } = require(
  root + "/src/controllers/handlers/http",
);
const viewer = "0x1111111111111111111111111111111111111111",
  owner = "0x2222222222222222222222222222222222222222";
const ids = [
  "00000000-0000-4000-8000-000000000001",
  "00000000-0000-4000-8000-000000000002",
  "00000000-0000-4000-8000-000000000003",
  "00000000-0000-4000-8000-000000000004",
];
const requests = ids.map(
  (id, index) => "00000000-0000-4000-8000-00000000100" + (index + 1),
);
const logs = {
  getLogger: () => ({ info() {}, debug() {}, warn() {}, error() {} }),
};
(async () => {
  const client = new Client({ connectionString: databaseUrl });
  await client.connect();
  const suffix = crypto.randomBytes(12).toString("hex"),
    schema = "request_pages_" + suffix,
    role = "request_pages_" + suffix,
    cases = [];
  try {
    await client.query(`CREATE SCHEMA ${schema}`);
    await client.query(`SET search_path TO ${schema}`);
    await client.query(
      `CREATE TABLE communities(id uuid PRIMARY KEY,name text,description text,owner_address text,private boolean,unlisted boolean,active boolean,editors_choice boolean DEFAULT false,ranking_score numeric DEFAULT 0);CREATE TABLE community_members(community_id uuid,member_address text,role text);CREATE TABLE community_bans(community_id uuid,banned_address text,active boolean);CREATE TABLE community_requests(id uuid PRIMARY KEY,community_id uuid,member_address text,type text,status text,created_at timestamp);CREATE TABLE friendships(id uuid PRIMARY KEY,address_requester text,address_requested text,is_active boolean,created_at timestamp DEFAULT now());`,
    );
    for (let i = 0; i < ids.length; i++) {
      await client.query(
        "INSERT INTO communities VALUES($1,$2,$3,$4,$5,$5,$6,false,$7)",
        [
          ids[i],
          ["Zulu visible", "Inactive", "Banning", "Alpha private"][i],
          "Description",
          owner,
          i === 3,
          i !== 1,
          i === 3 ? 100 : 0,
        ],
      );
      await client.query(
        "INSERT INTO community_members VALUES($1,$2,'owner')",
        [ids[i], owner],
      );
      await client.query(
        "INSERT INTO community_requests VALUES($1,$2,$3,'request_to_join','pending',$4)",
        [
          requests[i],
          ids[i],
          viewer,
          [
            "2024-01-01 03:00:00",
            "2024-01-01 05:00:00",
            "2024-01-01 04:00:00",
            "2024-01-01 02:00:00",
          ][i],
        ],
      );
    }
    await client.query("INSERT INTO community_bans VALUES($1,$2,true)", [
      ids[2],
      viewer,
    ]);
    await client.query(`CREATE ROLE ${role}`);
    await client.query(
      `GRANT USAGE ON SCHEMA ${schema} TO ${role}; GRANT SELECT ON ALL TABLES IN SCHEMA ${schema} TO ${role}`,
    );
    let trace = [];
    const query = async (q) => {
      trace.push({ text: q.text, values: q.values });
      return client.query(q.text, q.values);
    };
    const pg = {
      query,
      getCount: async (q) => Number((await query(q)).rows[0].count),
    };
    const db = createCommunitiesDBComponent({ logs, pg });
    const common = {
      logs,
      communitiesDb: db,
      registry: { getProfiles: async () => [] },
      communityOwners: { getOwnersNames: async () => ({ [owner]: "Owner" }) },
      commsGatekeeper: { getCommunitiesVoiceChatStatus: async () => ({}) },
    };
    const communities = createCommunityComponent(common),
      communityRequests = createCommunityRequestsComponent({
        ...common,
        communities,
      });
    for (const phase of [
      "healthy",
      "friendship-permission-error",
      "community-permission-error",
    ]) {
      if (phase === "friendship-permission-error")
        await client.query(`REVOKE SELECT ON friendships FROM ${role}`);
      if (phase === "community-permission-error")
        await client.query(
          `GRANT SELECT ON friendships TO ${role}; REVOKE SELECT ON communities FROM ${role}`,
        );
      await client.query(`SET ROLE ${role}`);
      for (const [version, handler] of [
        ["v1", getMemberRequestsHandler],
        ["v2", getMemberRequestsV2Handler],
      ])
        for (const [limit, offset] of [
          [2, 0],
          [2, 1],
          [3, 0],
          [3, 1],
          [2, 2],
          [2, 4],
        ]) {
          trace = [];
          const response = await handler({
            components: { logs, communityRequests },
            params: { address: viewer },
            verification: { auth: viewer },
            url: new URL(`http://social.test/?limit=${limit}&offset=${offset}`),
          });
          const expected = phase !== "healthy" && offset < 4 ? 500 : 200;
          if (response.status !== expected)
            throw Error(
              JSON.stringify({
                phase,
                version,
                limit,
                offset,
                response,
                trace,
              }),
            );
          cases.push({ phase, version, limit, offset, response, trace });
        }
      await client.query("RESET ROLE");
    }
    const paths = [
      "src/adapters/communities-db.ts",
      "src/adapters/pg.ts",
      "src/logic/queries.ts",
      "src/logic/community/communities.ts",
      "src/logic/community/requests.ts",
      "src/logic/community/utils.ts",
      "src/controllers/handlers/http/get-member-requests-handler.ts",
      "src/controllers/handlers/http/get-member-requests-v2-handler.ts",
      "src/utils/pagination.ts",
    ];
    fs.writeFileSync(
      process.argv[3],
      JSON.stringify(
        {
          source: "https://github.com/decentraland/social-service-ea",
          revision: SOCIAL_REVISION,
          runtime: process.version,
          postgres: (await client.query("SHOW server_version")).rows[0]
            .server_version,
          scope:
            "Actual member-request HTTP handlers, community components and SQL adapter execute against disposable minimal PostgreSQL tables. Scoped real SELECT permission errors exercise friendship SQL. Registry/owner/voice dependencies remain local fixtures; this does not capture an external deployment.",
          seed: {
            viewer,
            owner,
            communityIds: ids,
            requestIds: requests,
            rawOrder: [requests[1], requests[2], requests[0], requests[3]],
            visibleOrder: [requests[0], requests[3]],
          },
          sources: paths.map((path) => ({
            path,
            sha256: sha256(root, path),
          })),
          cases,
        },
        null,
        2,
      ) + "\n",
    );
  } finally {
    await client.query("RESET ROLE");
    await client.query(`DROP SCHEMA IF EXISTS ${schema} CASCADE`);
    await client.query(`DROP ROLE IF EXISTS ${role}`);
    await client.end();
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
