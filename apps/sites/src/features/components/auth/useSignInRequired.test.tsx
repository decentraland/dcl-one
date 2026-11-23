// @vitest-environment jsdom
import { StrictMode } from "react";
import { cleanup, render } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import { getIdentity } from "@/data/lib/auth/session";

import { openSignIn } from "./signin-store";
import { useSignInRequired } from "./useSignInRequired";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});
vi.mock("@/data/lib/auth/session", () => ({ getIdentity: vi.fn() }));
vi.mock("./signin-store", () => ({ openSignIn: vi.fn() }));

function ProtectedRoute({ required }: { required: boolean }) {
  useSignInRequired(required, false);
  return null;
}

function visit(url: string, required = true) {
  return render(
    <StrictMode>
      <MemoryRouter initialEntries={[url]}>
        <ProtectedRoute required={required} />
      </MemoryRouter>
    </StrictMode>,
  );
}

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("when a Create route requires sign-in", () => {
  it("should redirect an anonymous storage visitor once and preserve the editor URL", () => {
    vi.mocked(getIdentity).mockReturnValue(null);
    const redirectTo = "/create/storage?step=edit&world=example.dcl.eth&key=progress#panel";
    visit(redirectTo);
    expect(openSignIn).toHaveBeenCalledExactlyOnceWith({ redirectTo });
  });

  it("should redirect an expired session and preserve the selected project", () => {
    vi.mocked(getIdentity).mockReturnValue({ expiration: new Date(0).toISOString() } as ReturnType<typeof getIdentity>);
    const redirectTo = "/create/delete-project?step=confirm&project=scene-1";
    visit(redirectTo);
    expect(openSignIn).toHaveBeenCalledExactlyOnceWith({ redirectTo });
  });

  it("should wait for hydration when a valid session already exists", () => {
    vi.mocked(getIdentity).mockReturnValue({
      expiration: new Date(Date.now() + 60_000).toISOString(),
    } as ReturnType<typeof getIdentity>);
    visit("/create/storage");
    expect(openSignIn).not.toHaveBeenCalled();
  });

  it("should allow browsing project choices without requesting sign-in", () => {
    visit("/create/delete-project?step=scenes", false);
    expect(openSignIn).not.toHaveBeenCalled();
  });
});
