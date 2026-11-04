// Node26; actual source members/bans handlers/components/SQL over disposable PG.
// Usage: node <script> <source-root> <output> <postgres-url>. All profiles local.
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
const { createCommunityMembersComponent } = require(
  root + "/src/logic/community/members.ts",
);
const { createCommunityBansComponent } = require(
  root + "/src/logic/community/bans.ts",
);
const handlers = require(root + "/src/controllers/handlers/http");
const id = "00000000-0000-4000-8000-000000000001",
  friendship = "00000000-0000-4000-8000-000000000002",
  action = "00000000-0000-4000-8000-000000000003",
  viewer = "0x1111111111111111111111111111111111111111",
  peer = "0x2222222222222222222222222222222222222222";
const logs = {
  getLogger: () => ({ info() {}, warn() {}, debug() {}, error() {} }),
};
(async () => {
  const client = new Client({ connectionString: databaseUrl });
  await client.connect();
  const schema = "member_fields_" + crypto.randomBytes(12).toString("hex"),
    cases = [];
  try {
    await client.query(`CREATE SCHEMA ${schema}`);
    await client.query(`SET search_path TO ${schema}`);
    await client.query(
      `CREATE TABLE communities(id uuid PRIMARY KEY,name text,description text,owner_address text,private boolean,unlisted boolean,active boolean);CREATE TABLE community_members(community_id uuid,member_address text,role text,joined_at timestamptz);CREATE TABLE community_bans(community_id uuid,banned_address text,banned_by text,banned_at timestamptz,active boolean);CREATE TABLE friendships(id uuid PRIMARY KEY,address_requester text,address_requested text,is_active boolean,created_at timestamptz DEFAULT now());CREATE TABLE friendship_actions(id uuid PRIMARY KEY,friendship_id uuid,action text,acting_user text,timestamp timestamptz);`,
    );
    await client.query(
      "INSERT INTO communities VALUES($1,'Fixture','Description',$2,false,false,true)",
      [id, viewer],
    );
    await client.query(
      "INSERT INTO community_members VALUES($1,$2,'owner','2024-01-01T00:00:00Z'),($1,$3,'member','2024-01-01T00:00:01Z')",
      [id, viewer, peer],
    );
    await client.query(
      "INSERT INTO community_bans VALUES($1,$2,$3,'2024-01-01T00:00:00Z',true)",
      [id, peer, viewer],
    );
    let trace = [];
    const query = async (q) => {
      trace.push({ text: q.text, values: q.values });
      return client.query(q.text, q.values);
    };
    const pg = {
      query,
      getCount: async (q) => Number((await query(q)).rows[0].count),
      exists: async (q, key) => (await query(q)).rows[0]?.[key] ?? false,
    };
    const db = createCommunitiesDBComponent({ logs, pg }),
      registry = {
        getProfiles: async (addresses) =>
          addresses.map((address) => ({
            avatars: [
              {
                userId: address,
                name: "Profile",
                hasClaimedName: true,
                nameColor: { r: 0.5, g: 0.25, b: 1 },
                avatar: { snapshots: { face256: "bafkface" } },
              },
            ],
          })),
      };
    const common = {
      logs,
      communitiesDb: db,
      registry,
      communityRoles: { validatePermissionToGetBannedMembers: async () => {} },
    };
    const communityMembers = await createCommunityMembersComponent(common),
      communityBans = await createCommunityBansComponent(common);
    for (const phase of [
      "accepted",
      "request-sent",
      "request-received",
      "none",
      "empty-history",
    ]) {
      await client.query("TRUNCATE friendships,friendship_actions");
      if (phase !== "none")
        await client.query(
          "INSERT INTO friendships(id,address_requester,address_requested,is_active) VALUES($1,$2,$3,$4)",
          [friendship, viewer, peer, phase === "accepted"],
        );
      if (!["none", "empty-history"].includes(phase))
        await client.query(
          "INSERT INTO friendship_actions VALUES($1,$2,$3,$4,'2024-01-02T00:00:00Z')",
          [
            action,
            friendship,
            phase === "accepted" ? "accept" : "request",
            phase === "request-sent" ? viewer : peer,
          ],
        );
      const routes = [
        ["/v1/communities/:id/members", "getCommunityMembersHandler"],
        ["/v2/communities/:id/members", "getCommunityMembersV2Handler"],
        ["/v1/communities/:id/bans", "getBannedMembersHandler"],
        ["/v2/communities/:id/bans", "getBannedMembersV2Handler"],
        ...(phase === "accepted"
          ? [
              [
                "/v1/communities/:id/members",
                "getCommunityMembersHandler",
                true,
              ],
              [
                "/v2/communities/:id/members",
                "getCommunityMembersV2Handler",
                true,
              ],
            ]
          : []),
      ];
      for (const [path, handler, anonymous = false] of routes) {
        trace = [];
        const response = await handlers[handler]({
          components: {
            logs,
            config: { getString: async () => undefined },
            communityMembers,
            communityBans,
          },
          params: { id },
          verification: anonymous ? undefined : { auth: viewer },
          request: new Request("http://social.test/"),
          url: new URL("http://social.test/"),
        });
        if (response.status !== 200)
          throw Error(JSON.stringify({ phase, path, response, trace }));
        const row = response.body.data.results.find(
          (row) => row.memberAddress === peer,
        );
        const expose = path.startsWith("/v1/") && !anonymous;
        if (
          Object.hasOwn(row, "lastFriendshipAction") !== expose ||
          Object.hasOwn(row, "actingUser") !== expose
        )
          throw Error(
            "Alias presence mismatch " +
              JSON.stringify({ phase, path, anonymous, row }),
          );
        cases.push({ phase, path, handler, anonymous, response, trace });
      }
    }
    const paths = [
      "src/adapters/communities-db.ts",
      "src/adapters/pg.ts",
      "src/logic/queries.ts",
      "src/logic/community/members.ts",
      "src/logic/community/bans.ts",
      "src/logic/community/utils.ts",
      "src/logic/profiles.ts",
      "src/logic/friends/friends.ts",
      "src/controllers/handlers/http/get-community-members-handlers.ts",
      "src/controllers/handlers/http/get-community-members-v2-handler.ts",
      "src/controllers/handlers/http/get-banned-members-handler.ts",
      "src/controllers/handlers/http/get-banned-members-v2-handler.ts",
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
            "Actual members/bans HTTP handlers, components and SQL adapter execute against disposable minimal PostgreSQL tables. Local registry profiles exercise response mapping. V1 signed aliases retain strings or SQL null; anonymous members and V2 omit them. No claim about external deployments or unrelated DTO families.",
          seed: { id, viewer, peer },
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
    await client.query(`DROP SCHEMA IF EXISTS ${schema} CASCADE`);
    await client.end();
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
