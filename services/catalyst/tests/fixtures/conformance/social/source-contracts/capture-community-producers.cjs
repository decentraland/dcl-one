// Executes all twelve source event creators with local dependency responses.
const fs = require("fs"),
  root = process.argv[2];
require(root + "/node_modules/ts-node/register/transpile-only");
const { createCommunityComponent } = require(
  root + "/src/logic/community/communities.ts",
);
const { createCommunityMembersComponent } = require(
  root + "/src/logic/community/members.ts",
);
const { createCommunityBansComponent } = require(
  root + "/src/logic/community/bans.ts",
);
const { createCommunityPostsComponent } = require(
  root + "/src/logic/community/posts.ts",
);
const { createCommunityRequestsComponent } = require(
  root + "/src/logic/community/requests.ts",
);
const { createCommunityVoiceComponent } = require(
  root + "/src/logic/community-voice/community-voice.ts",
);
const id = "00000000-0000-4000-8000-000000000001",
  postId = "00000000-0000-4000-8000-000000000002",
  requestId = "00000000-0000-4000-8000-000000000003";
const address = (n) => "0x" + n.toString(16).padStart(40, "0"),
  owner = address(1),
  member = address(4),
  timestamp = 1793721600000;
const noop = async () => {},
  proxy = (overrides) =>
    new Proxy(overrides, {
      get: (object, key) =>
        key === "then" ? undefined : (object[key] ?? noop),
    });
const base = {
  id,
  name: "Fixture community",
  description: "Description",
  ownerAddress: owner,
  privacy: "public",
  visibility: "all",
  active: true,
  role: "owner",
};
const events = [];
let operation = "";
const common = {
  logs: { getLogger: () => ({ info() {}, error() {}, warn() {}, debug() {} }) },
  communityBroadcaster: {
    broadcast: async (event, options) =>
      events.push({
        operation,
        event: JSON.parse(JSON.stringify(event)),
        ...(options ? { options } : {}),
      }),
  },
  communityRoles: proxy({}),
  communityThumbnail: {
    buildThumbnailUrl: (id) =>
      "http://cdn.test/social/communities/" + id + "/raw-thumbnail.png",
    getThumbnail: async () => undefined,
  },
  analytics: { fireEvent: noop },
  pubsub: { publishInChannel: noop },
  catalystClient: { getOwnedNames: async () => [{ name: "Fixture" }] },
  registry: {
    getProfile: async () => ({
      avatars: [
        {
          userId: member,
          name: "Member",
          hasClaimedName: true,
          avatar: {
            snapshots: { face256: "http://images.example.org/face.png" },
          },
        },
      ],
    }),
  },
};
const db = proxy({
  getCommunity: async () => base,
  communityExists: async () => true,
  isMemberOfCommunity: async () => true,
  isMemberBanned: async () => false,
  updateCommunity: async (_, update) => ({ ...base, ...update }),
  getCommunityPlaces: async () => [],
  getCommunityMemberRole: async () => "owner",
  createPost: async (input) => ({
    ...input,
    id: postId,
    createdAt: new Date(timestamp),
  }),
});
const flush = () => new Promise((resolve) => setImmediate(resolve));
(async () => {
  Date.now = () => timestamp;
  const community = createCommunityComponent({
    ...common,
    communitiesDb: db,
    featureFlags: { getVariants: async () => [address(3)] },
    communityComplianceValidator: { validateCommunityContent: noop },
  });
  operation = "delete-by-global-moderator";
  await community.deleteCommunity(id, address(3));
  await flush();
  operation = "rename";
  await community.updateCommunity(id, owner, { name: "After" });
  await flush();
  const members = await createCommunityMembersComponent({
    ...common,
    communitiesDb: db,
  });
  operation = "kick";
  await members.kickMember(id, owner, member);
  await flush();
  operation = "leave";
  await members.leaveCommunity(id, member);
  await flush();
  operation = "transfer";
  await members.updateMemberRole(id, owner, member, "owner");
  await flush();
  const bans = await createCommunityBansComponent({
    ...common,
    communitiesDb: db,
  });
  operation = "ban";
  await bans.banMember(id, owner, member);
  await flush();
  const posts = createCommunityPostsComponent({ ...common, communitiesDb: db });
  operation = "post";
  await posts.createPost(id, owner, "Content");
  await flush();
  const request = {
    id: requestId,
    communityId: id,
    memberAddress: member,
    type: "request_to_join",
    status: "pending",
  };
  const requestDb = proxy({
    ...db,
    getCommunity: async () => ({ ...base, privacy: "private" }),
    getCommunityMemberRole: async () => "none",
    getCommunityRequests: async () => [],
    getCommunityRequest: async () => request,
    createCommunityRequest: async (_, __, type) => ({ ...request, type }),
  });
  const requests = createCommunityRequestsComponent({
    ...common,
    communitiesDb: requestDb,
  });
  operation = "request";
  await requests.createCommunityRequest(id, member, "request_to_join", member);
  await flush();
  operation = "invite";
  await requests.createCommunityRequest(id, member, "invite", owner);
  await flush();
  operation = "accept-request";
  await requests.updateRequestStatus(requestId, "accepted", {
    callerAddress: owner,
  });
  await flush();
  const voice = await createCommunityVoiceComponent({
    ...common,
    communitiesDb: db,
    commsGatekeeper: {
      getCommunityVoiceChatStatus: async () => ({ isActive: false }),
      createCommunityVoiceChatRoom: async () => ({
        connectionUrl: "http://media.example.org",
      }),
    },
    communityVoiceChatCache: { setCommunityVoiceChat: noop },
  });
  operation = "start-voice";
  // Room creation time belongs to the key; the event samples its own clock
  // after optional enrichment. Keep those observably distinct in this capture.
  let voiceClock = timestamp;
  Date.now = () => voiceClock++;
  await voice.startCommunityVoiceChat(id, owner, { includeProfile: false });
  await flush();
  fs.writeFileSync(
    process.argv[3],
    JSON.stringify(
      {
        source: "decentraland/social-service-ea",
        revision: "b6b2cefefdf727b2ed47ddb887e77d226a41eea6",
        node: process.version,
        sources: [
          "src/logic/community/communities.ts",
          "src/logic/community/members.ts",
          "src/logic/community/bans.ts",
          "src/logic/community/posts.ts",
          "src/logic/community/requests.ts",
          "src/logic/community-voice/community-voice.ts",
        ],
        timestamp,
        events,
      },
      null,
      2,
    ) + "\n",
  );
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
