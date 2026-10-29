import type { ReactNode } from "react";
import { Link } from "react-router";
import { href } from "@/core/lib/router/routes";

import CreatorHubChrome from "@ui/creatorhub/frames/CreatorHubChrome";
import CreatorHubBreadcrumb from "@ui/creatorhub/components/CreatorHubBreadcrumb";
import { PageHeader } from "@ui/components/Surface";
import "@ui/creatorhub/frames/creatorhubchrome.css";
import "@ui/creatorhub/pages/chmanage.css";
import { useAuth } from "@/data/lib/auth/index";
import { openSignIn } from "@/features/components/auth/signin-store";
import {
  ManageLoading,
  ManageSignIn,
  useManageScope,
} from "@/features/components/creator-hub/manage-scope";
import { useChromeAuth } from "@ui/web/frames/chrome-auth";
import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoaderWith } from "@/core/lib/telemetry/page-loader";

import {
  loadWorldPermissions,
  type LoadWorldPermissionsResult,
} from "@/data/lib/catalyst/creator-hub/world-permissions.server";
import { emptyWorldPermissions, viewerWorldRole, worldExists } from "@/data/lib/catalyst/creator-hub/world-permissions";
import { loadWorldScenes } from "@/data/lib/catalyst/creator-hub/world-scenes.server";
import { loadManageWorlds } from "@/data/lib/catalyst/creator-hub/manage-worlds.server";
import { readWallet } from "@/data/lib/auth/wallet-cookie";
import WorldPermissionsWizard from "@/features/creator-hub/world-permissions/WorldPermissionsWizard";
import type { CommitFn } from "@/features/creator-hub/world-permissions/machine";
import { commitWorldAccess } from "@/data/lib/catalyst/creator-hub/world-permissions-commit";
import {
  WorldGateView,
  type WorldGate,
} from "@/features/components/creator-hub/world-gate";

import { creatorHubMeta } from "@/core/lib/seo/creator-hub-meta";

import type { Route } from "./+types/create.admissions";
import type { StoryId } from "@/core/lib/telemetry/story-id";

export const meta = () => creatorHubMeta("Admissions");

const STORY: StoryId = "creator-hub/world-permissions";

const FALLBACK: PageDefaults = {
  variant: "wizard",
};

type PickableWorld = { name: string; role: string; deployedScenes: number };

export async function loader({ request }: Route.LoaderArgs) {
  const url = new URL(request.url);
  const step = url.searchParams.get("step")?.trim() || null;
  const worldName = url.searchParams.get("world")?.trim() || "";
  const noWorld = worldName === "";

  const { sid, assignment, wrap, data: access } = await pageLoaderWith(
    request,
    
    FALLBACK,
    () => noWorld ? Promise.resolve(null) : Promise.all([
      loadWorldPermissions(worldName, { signal: request.signal }).catch(() => null),
      loadWorldScenes(worldName, { signal: request.signal }),
    ]),
  );

  const viewer = (
    url.searchParams.get("address")?.trim() ||
    readWallet(request) ||
    ""
  ).toLowerCase();

  // Reached without a world (the sidebar entry): offer the viewer's Worlds to pick from.
  const pick: PickableWorld[] = noWorld && viewer
    ? ((await loadManageWorlds(viewer, request.signal).catch(() => null))?.worlds ?? [])
        .map((w) => ({ name: w.name, role: w.role, deployedScenes: w.deployedScenes }))
    : [];

  let perms: LoadWorldPermissionsResult | null = null;
  let gate: WorldGate = noWorld ? "no-world" : "none";
  if (access) {
    const [loaded, scenes] = access;
    perms =
      loaded ?? {
        permissions: emptyWorldPermissions(worldName),
        source: "empty",
        fallback: true,
      };
    if (perms.fallback || scenes === null) {
      gate = "load-failed";
    } else if (!worldExists(perms.permissions, scenes.length)) {
      gate = "not-found";
    } else if (viewerWorldRole(perms.permissions, viewer) === "none") {
      gate = "no-access";
    }
  }

  const payload = {
    sid,
    step,
    worldName,
    noWorld,
    pick,
    gate,
    viewer,
    assignment,
    permissions: perms?.permissions ?? null,
    source: perms?.source ?? null,
    fallback: perms?.fallback ?? false,
  };
  return wrap(payload);
}

export default function CreatorHubWorldPermissions({
  loaderData,
}: Route.ComponentProps) {
  const { sid, step, assignment, permissions, gate, viewer, worldName, noWorld, pick } =
    loaderData;
  const { isConnected, address, identity } = useAuth();
  const { name } = useChromeAuth();
  const commit: CommitFn | undefined =
    identity && worldName
      ? async ({ accessType, collaborators, signal }) => {
          await commitWorldAccess(
            worldName,
            { accessType, collaborators },
            { identity, signal },
          );
          return { aclWritten: true, addresses: collaborators.length };
        }
      : undefined;
  const { rescoping, needsConnect } = useManageScope(viewer);

  let body: ReactNode;
  if (noWorld) {
    body = rescoping ? (
      <Framed><ManageLoading label={"Loading your Worlds\u{2026}"} /></Framed>
    ) : needsConnect ? (
      <Framed>
        <ManageSignIn
          icon={AdmissionsGlyph}
          title="Sign in to manage admissions"
          subtitle="Decide who can enter each World you own and who holds each permission on it."
        />
      </Framed>
    ) : pick.length > 0 ? (
      <WorldPicker worlds={pick} viewer={viewer} />
    ) : (
      <WorldGateView
        gate="no-world"
        worldName=""
        surface="permissions"
        checkingAccess={false}
        signedIn={isConnected}
        onSignIn={() => openSignIn()}
      />
    );
  } else if (gate === "none" && permissions && !commit && !isConnected) {
    body = (
      <Framed>
        <ManageSignIn
          icon={AdmissionsGlyph}
          title={`Sign in to manage admissions to ${worldName}`}
          subtitle="Changing who can enter is a signed request from the World's owner or a permission holder."
        />
      </Framed>
    );
  } else if (gate !== "none" || !permissions || !commit) {
    body = (
      <WorldGateView
        gate={gate === "none" ? "no-world" : gate}
        worldName={worldName}
        surface="permissions"
        checkingAccess={rescoping || (gate === "none" && !!permissions && !commit)}
        signedIn={isConnected}
        onSignIn={() => openSignIn()}
      />
    );
  } else {
    body = (
      <WorldPermissionsWizard
        trackCtx={{
          sid,
          story: STORY,
          variant: assignment.variant,
        }}
        data={permissions}
        initialStep={step ?? undefined}
        commit={commit}
      />
    );
  }

  return (
    <CreatorHubChrome
      active="admissions"
      signedIn={isConnected}
      account={address ?? ""}
      name={name}
      onSignIn={() => openSignIn()}
    >
      <CreatorHubBreadcrumb to={href("/create/worlds")} label="Back to Worlds" LinkComponent={Link} />

      <section className="creator-hub-world-permissions">{body}</section>
    </CreatorHubChrome>
  );
}

const AdmissionsGlyph = (
  <svg viewBox="0 0 20 20" width="28" height="28" fill="none" aria-hidden="true">
    <rect x="4" y="9" width="12" height="8" rx="1.5" stroke="currentColor" strokeWidth="1.5" />
    <path d="M7 9V6.5a3 3 0 0 1 6 0V9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);

function Framed({ children }: { children: ReactNode }) {
  return (
    <div className="chm">
      <div className="chm__container">{children}</div>
    </div>
  );
}

function WorldPicker({ worlds, viewer }: { worlds: PickableWorld[]; viewer: string }) {
  return (
    <div className="chm">
      <div className="chm__container">
        <PageHeader
          title="Admissions"
          description="Decide who can enter each World: anyone, a password, or a list of people and communities. Pick a World to begin."
        />
        <div className="chm__list">
          {worlds.map((world) => (
            <div key={world.name} className="chm__card chm__card--compact">
              <div className="chm__cardhead chm__cardhead--chips">
                <span className="chm__cardtitle u-truncate">{world.name}</span>
                <span className="chm__chips">
                  <span className="chm__chip">{world.role === "owner" ? "Owner" : "Collaborator"}</span>
                </span>
              </div>
              <div className="chm__cardbody">
                <span className="chm__publabel">World</span>
                <span className="chm__projtitle">
                  {world.deployedScenes > 0
                    ? `${world.deployedScenes} ${world.deployedScenes === 1 ? "scene" : "scenes"} published`
                    : "Not published yet"}
                </span>
                <div className="chm__cardactions">
                  <Link
                    to={`/create/admissions?world=${encodeURIComponent(world.name)}&address=${encodeURIComponent(viewer)}`}
                    prefetch="intent"
                    className="chm__settingsbtn"
                    style={{ textDecoration: "none" }}
                  >
                    Who can enter
                  </Link>
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
