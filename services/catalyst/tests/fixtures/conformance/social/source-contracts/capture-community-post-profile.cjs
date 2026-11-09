// Actual registry, post component and HTTP handler, with local dependency responses.
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
const { createCommunityPostsComponent } = require(
  root + "/src/logic/community/posts.ts",
);
const { createCommunityPostHandler } = require(
  root + "/src/controllers/handlers/http/create-community-post-handler.ts",
);
const author = "0x9222222222222222222222222222222222222222";
const alias = "0x9333333333333333333333333333333333333333";
const id = "00000000-0000-4000-8000-000000000001";
const postId = "00000000-0000-4000-8000-000000000002";
const timestamp = 1793721600000;
const logs = {
  getLogger: () => ({ info() {}, warn() {}, error() {}, debug() {} }),
};
(async () => {
  const originalNow = Date.now;
  try {
  Date.now = () => timestamp;
  const cases = [];
  for (const name of ["regular", "alias", "missing", "invalid"]) {
    const documents =
      name === "missing"
        ? []
        : [
            {
              avatars: [
                {
                  ...(name === "invalid"
                    ? {}
                    : { userId: name === "alias" ? alias : author }),
                  name: "Source author",
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
    const order = [],
      cache = new Map(),
      events = [];
    const registry = await createRegistryComponent({
      logs,
      config: { requireString: async () => "https://registry.example.org" },
      fetcher: {
        fetch: async () => {
          order.push("profile");
          return new Response(JSON.stringify(documents));
        },
      },
      redis: {
        get: async (key) => cache.get(key),
        put: async (key, value) => cache.set(key, value),
      },
    });
    const communityPosts = createCommunityPostsComponent({
      logs,
      registry,
      communityRoles: { validatePermissionToCreatePost: async () => {} },
      communitiesDb: {
        getCommunity: async () => ({ id, name: "Fixture community" }),
        createPost: async (input) => {
          order.push("insert");
          return { ...input, id: postId, createdAt: new Date(timestamp) };
        },
      },
      communityThumbnail: { buildThumbnailUrl: () => "N/A" },
      communityBroadcaster: {
        broadcast: async (event) => {
          order.push("broadcast");
          events.push(event);
        },
      },
    });
    const response = await createCommunityPostHandler({
      components: { logs, communityPosts },
      params: { id },
      verification: { auth: author },
      request: new Request(
        "http://social.test/v1/communities/" + id + "/posts",
        { method: "POST", body: JSON.stringify({ content: "Content" }) },
      ),
    });
    await new Promise((resolve) => setImmediate(resolve));
    cases.push({ name, documents, order, response, events });
  }
  fs.writeFileSync(
    process.argv[3],
    JSON.stringify(
      {
        source: "decentraland/social-service-ea",
        revision: SOCIAL_REVISION,
        node: process.version,
        sources: [
          "src/adapters/registry.ts",
          "src/logic/community/posts.ts",
          "src/controllers/handlers/http/create-community-post-handler.ts",
        ],
        author,
        alias,
        timestamp,
        cases,
      },
      null,
      2,
    ) + "\n",
  );
  } finally { Date.now = originalNow; }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
