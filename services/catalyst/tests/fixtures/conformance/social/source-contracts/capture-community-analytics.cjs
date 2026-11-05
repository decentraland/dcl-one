// Actual source mutators, with local DB/provider dependencies and analytics capture.
const { sourceCapture, SOCIAL_REVISION, sha256 } = require(
  require("node:path").join(
    require("node:child_process").execFileSync("git", ["-C", __dirname, "rev-parse", "--show-toplevel"], { encoding: "utf8" }).trim(),
    "tooling/social-capture-common.cjs",
  ),
);
const capture = sourceCapture(process.argv[2]);
const fs = require("fs"),
  root = capture.upstream;
const { createCommunityComponent } = require(
  root + "/src/logic/community/communities.ts",
);
const { createCommunityMembersComponent } = require(
  root + "/src/logic/community/members.ts",
);
const { createCommunityBansComponent } = require(
  root + "/src/logic/community/bans.ts",
);
const { createCommunityRequestsComponent } = require(
  root + "/src/logic/community/requests.ts",
);
const id = "AABBCCDD-0000-4000-8000-000000000001",
  requestId = "bbeeffaa-0000-4000-8000-000000000002",
  otherRequestId = "00000000-0000-4000-8000-000000000003",
  owner = "0x1111111111111111111111111111111111111111",
  member = "0x2222222222222222222222222222222222222222";
const noop = async () => {},
  base = {
    id,
    name: "Fixture community",
    description: "Description",
    ownerAddress: owner,
    privacy: "public",
    visibility: "all",
    active: true,
    role: "owner",
  };
const cases = [];
(async () => {
  for (const name of [
    "join",
    "join-no-request",
    "join-existing",
    "join-write-failure",
    "kick",
    "kick-nonmember",
    "leave",
    "leave-nonmember",
    "ban",
    "ban-nonmember",
    "ban-later-write-failure",
    "privacy-accept-all",
    "privacy-no-requests",
    "opposite-request",
    "duplicate-request",
    "accept-request",
    "reject-request",
    "reject-uppercase-request",
    "cancel-request",
  ]) {
    const trace = [],
      analytics = [];
    const request = {
      id: requestId,
      communityId: id.toLowerCase(),
      memberAddress: member,
      type: "request_to_join",
      status: "pending",
    };
    const common = {
      logs: {
        getLogger: () => ({ info() {}, error() {}, debug() {}, warn() {} }),
      },
      communityRoles: new Proxy({}, { get: () => noop }),
      analytics: {
        fireEvent: (event, properties) => {
          trace.push("analytics:" + event);
          analytics.push({
            event,
            properties: JSON.parse(JSON.stringify(properties)),
          });
        },
      },
      communityBroadcaster: { broadcast: noop },
      communityThumbnail: {
        buildThumbnailUrl: () => "http://cdn.test/raw.png",
        getThumbnail: noop,
      },
      pubsub: {
        publishInChannel: async () => {
          trace.push("pubsub");
        },
      },
      commsGatekeeper: { kickUserFromCommunityVoiceChat: noop },
      communityComplianceValidator: { validateCommunityContent: noop },
      featureFlags: { getVariants: async () => [owner] },
    };
    const overrides = {
      getCommunity: async () => ({
        ...base,
        privacy:
          name.startsWith("privacy-") ||
          [
            "opposite-request",
            "duplicate-request",
            "accept-request",
            "reject-request",
            "cancel-request",
          ].includes(name)
            ? "private"
            : "public",
      }),
      communityExists: async () => true,
      isMemberOfCommunity: async () =>
        name === "join-existing" ||
        (!name.startsWith("join") &&
          !name.endsWith("nonmember") &&
          !name.endsWith("-request")),
      isMemberBanned: async () => false,
      joinMemberAndRemoveRequests: async () => {
        trace.push("db:join");
        if (name === "join-write-failure") throw Error("fixture join failure");
        return name === "join-no-request" ? undefined : requestId;
      },
      kickMemberFromCommunity: async () => {
        trace.push("db:kick");
      },
      unlikePostsFromCommunity: async () => {
        trace.push("db:unlike");
      },
      banMemberFromCommunity: async () => {
        trace.push("db:ban");
        if (name === "ban-later-write-failure")
          throw Error("fixture ban failure");
      },
      removeMemberRequests: async () => {
        trace.push("db:remove-member-requests");
      },
      updateCommunity: async (_, update) => ({ ...base, ...update }),
      acceptAllRequestsToJoin: async () => {
        trace.push("db:accept-all");
        return name === "privacy-no-requests"
          ? []
          : [requestId, otherRequestId];
      },
      getCommunityMemberRole: async () =>
        name === "opposite-request" || name === "duplicate-request"
          ? "none"
          : "owner",
      getCommunityRequests: async () =>
        name === "opposite-request"
          ? [{ ...request, type: "invite" }]
          : name === "duplicate-request"
            ? [request]
            : [],
      getCommunityRequest: async () => request,
      removeCommunityRequest: async () => {
        trace.push("db:remove-request");
      },
    };
    const db = new Proxy(overrides, {
      get: (object, key) =>
        key === "then" ? undefined : (object[key] ?? noop),
    });
    const members = await createCommunityMembersComponent({
      ...common,
      communitiesDb: db,
    });
    const bans = await createCommunityBansComponent({
      ...common,
      communitiesDb: db,
    });
    const communities = createCommunityComponent({
      ...common,
      communitiesDb: db,
    });
    const requests = createCommunityRequestsComponent({
      ...common,
      communitiesDb: db,
    });
    let error;
    try {
      if (name.startsWith("join")) await members.joinCommunity(id, member);
      else if (name.startsWith("kick"))
        await members.kickMember(id, owner, member);
      else if (name.startsWith("leave"))
        await members.leaveCommunity(id, member);
      else if (name.startsWith("ban")) await bans.banMember(id, owner, member);
      else if (name.startsWith("privacy-"))
        await communities.updateCommunity(id, owner, { privacy: "public" });
      else if (name === "opposite-request" || name === "duplicate-request")
        await requests.createCommunityRequest(
          id,
          member,
          "request_to_join",
          member,
        );
      else
        await requests.updateRequestStatus(
          name === "reject-uppercase-request"
            ? requestId.toUpperCase()
            : requestId,
          name === "accept-request"
            ? "accepted"
            : name === "reject-request" || name === "reject-uppercase-request"
              ? "rejected"
              : "cancelled",
          { callerAddress: name === "cancel-request" ? member : owner },
        );
    } catch (failure) {
      error = failure.message;
    }
    await new Promise((resolve) => setImmediate(resolve));
    cases.push({ name, analytics, trace, ...(error ? { error } : {}) });
  }
  const adapterCalls = [];
  const { createCommunitiesDBComponent } = require(
    root + "/src/adapters/communities-db.ts",
  );
  const adapter = createCommunitiesDBComponent({
    logs: {
      getLogger: () => ({ info() {}, error() {}, debug() {}, warn() {} }),
    },
    pg: {
      withTransaction: async (work) =>
        work({
          query: async (sql, values) => {
            const text = typeof sql === "string" ? sql : sql.text;
            adapterCalls.push({
              sql: text.replace(/\s+/g, " ").trim(),
              values: values || sql.values,
            });
            return {
              rows: text.includes("RETURNING id")
                ? [{ id: requestId }, { id: otherRequestId }]
                : [],
            };
          },
        }),
    },
  });
  const adapterResults = {
    acceptAll: await adapter.acceptAllRequestsToJoin(id),
    join: await adapter.joinMemberAndRemoveRequests({
      communityId: id.toLowerCase(),
      memberAddress: member,
      role: "member",
    }),
  };
  const sources = [
    "src/types/analytics.ts",
    "src/logic/community/members.ts",
    "src/logic/community/bans.ts",
    "src/logic/community/communities.ts",
    "src/logic/community/requests.ts",
    "src/adapters/communities-db.ts",
  ];
  fs.writeFileSync(
    process.argv[3],
    JSON.stringify(
      {
        source: "decentraland/social-service-ea",
        revision: SOCIAL_REVISION,
        node: process.version,
        sources: sources.map((path) => ({
          path,
          sha256: sha256(root, path),
        })),
        scope:
          "Actual seven source community analytics hook locations; domain dependencies are local mocks. Emission and DB/pubsub call order, no-ops and selected DB failures are captured. The SDK transport is covered separately; this capture does not establish database atomicity.",
        id,
        requestId,
        otherRequestId,
        owner,
        member,
        cases,
        adapterCalls,
        adapterResults,
      },
      null,
      2,
    ) + "\n",
  );
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
