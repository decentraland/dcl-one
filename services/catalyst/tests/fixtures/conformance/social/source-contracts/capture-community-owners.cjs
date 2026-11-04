// Executes the pinned registry, owner component, community component and HTTP handlers.
// Usage: TS_NODE_COMPILER_OPTIONS='{"module":"CommonJS","moduleResolution":"node"}' node <script> <social-root> <output-json>
const { sourceCapture, SOCIAL_REVISION } = require(
  require("node:path").join(
    require("node:child_process").execFileSync("git", ["-C", __dirname, "rev-parse", "--show-toplevel"], { encoding: "utf8" }).trim(),
    "tooling/social-capture-common.cjs",
  ),
);
const capture = sourceCapture(process.argv[2]);
const fs = require("fs");
const root = capture.upstream;
const { createRegistryComponent } = require(root + "/src/adapters/registry.ts");
const { createCommunityOwnersComponent } = require(
  root + "/src/logic/community/owners.ts",
);
const { createCommunityComponent } = require(
  root + "/src/logic/community/communities.ts",
);
const { getCommunityHandler } = require(
  root + "/src/controllers/handlers/http/get-community-handler.ts",
);
const { createCommunityHandler } = require(
  root + "/src/controllers/handlers/http/create-community-handler.ts",
);
const owner = "0x9222222222222222222222222222222222222222";
const alias = "0x9333333333333333333333333333333333333333";
const id = "00000000-0000-4000-8000-000000000001";
const logs = {
  getLogger: () => ({ info() {}, warn() {}, error() {}, debug() {} }),
};
(async () => {
  const cases = [];
  for (const name of ["alias", "regular", "missing", "invalid"]) {
    const documents =
      name === "missing"
        ? []
        : [
            {
              avatars: [
                {
                  ...(name === "invalid"
                    ? {}
                    : { userId: name === "alias" ? alias : owner }),
                  name: "Source profile",
                  hasClaimedName: true,
                  avatar: {
                    snapshots: {
                      face256: "https://images.example.org/raw-face.png",
                    },
                  },
                },
              ],
            },
          ];
    const cache = new Map();
    const registry = await createRegistryComponent({
      logs,
      config: { requireString: async () => "https://registry.example.org" },
      fetcher: {
        fetch: async () =>
          new Response(JSON.stringify(documents), { status: 200 }),
      },
      redis: {
        get: async (key) => cache.get(key),
        mGet: async (keys) => keys.map((key) => cache.get(key)),
        put: async (key, value) => cache.set(key, value),
      },
    });
    const owners = createCommunityOwnersComponent({ registry });
    const base = {
      id,
      name: "Fixture community",
      description: "seeded community",
      ownerAddress: owner,
      privacy: "public",
      active: true,
      membersCount: 1,
    };
    const communities = createCommunityComponent({
      logs,
      registry,
      communityOwners: owners,
      catalystClient: { getOwnedNames: async () => [{ name: "Fixture" }] },
      commsGatekeeper: {
        getCommunityVoiceChatStatus: async () => ({
          isActive: false,
          participantCount: 0,
          moderatorCount: 0,
        }),
      },
      communityComplianceValidator: {
        validateCommunityContent: async () => {},
      },
      communitiesDb: {
        getCommunityPublicInformation: async () => base,
        createCommunity: async (data) => ({
          id,
          name: data.name,
          description: data.description,
          ownerAddress: owner,
          privacy: "public",
          visibility: "all",
          active: true,
        }),
        addCommunityMember: async () => {},
      },
    });
    const get = await getCommunityHandler({
      components: { logs, communities },
      params: { id },
    });
    const create = await createCommunityHandler({
      components: {
        logs,
        communities,
        communityFieldsValidator: {
          validate: async () => ({
            name: "Fixture created community",
            description: "fixture description",
            privacy: "public",
            visibility: "all",
            placeIds: [],
          }),
        },
      },
      verification: { auth: owner },
      formData: { fields: {}, files: {} },
    });
    cases.push({ name, documents, get, create });
  }
  fs.writeFileSync(
    process.argv[3],
    JSON.stringify(
      {
        source: "decentraland/social-service-ea",
        revision: SOCIAL_REVISION,
        node: process.version,
        owner,
        alias,
        sources: [
          "src/adapters/registry.ts",
          "src/logic/community/owners.ts",
          "src/logic/community/communities.ts",
          "src/controllers/handlers/http/get-community-handler.ts",
          "src/controllers/handlers/http/create-community-handler.ts",
        ],
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
