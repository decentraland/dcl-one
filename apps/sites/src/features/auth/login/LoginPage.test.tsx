// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { hydrateRoot } from "react-dom/client";
import { renderToString } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { getIdentity, setIdentity } from "@/data/lib/auth/session";

import LoginPage from "./LoginPage";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});

const auth = vi.hoisted(() => ({
  inAppAvailable: true,
  startEmailSignIn: vi.fn(async (_email: string) => {}),
  verifyEmailSignIn: vi.fn(async (_email: string, _code: string) => ({ signer: "0xabc" }) as unknown),
  startSocialSignIn: vi.fn(),
  connectWalletConnect: vi.fn(),
}));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => auth }));
const social = vi.hoisted(() => ({ fail: false }));
vi.mock("@/data/lib/auth/thirdweb/api", async (original) => ({
  ...(await original<typeof import("@/data/lib/auth/thirdweb/api")>()),
  socialLoginUrl: (provider: string, redirectUrl: string) => {
    if (social.fail) throw new Error("Social sign-in is unavailable.");
    return `https://social.example/${provider}?redirectUrl=${encodeURIComponent(redirectUrl)}`;
  },
}));

const SIGNER = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const assign = vi.fn();

type Request = { method: string; params?: unknown[] };
function installWallet(answer: (req: Request) => unknown) {
  const calls: string[] = [];
  (window as { ethereum?: unknown }).ethereum = {
    isMetaMask: true,
    request: vi.fn(async (req: Request) => {
      calls.push(req.method);
      return answer(req);
    }),
  };
  return calls;
}

function page(redirectTo: string | null = "/shop/items?sortBy=newest", layoutVariant: string | null = "two-options") {
  return render(<LoginPage redirectTo={redirectTo} layoutVariant={layoutVariant} />);
}

beforeEach(() => {
  vi.stubGlobal("location", {
    href: "http://localhost:3000/auth/login",
    origin: "http://localhost:3000",
    pathname: "/auth/login",
    search: "",
    hash: "",
    assign,
  });
  localStorage.clear();
  setIdentity(null);
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
  delete (window as { ethereum?: unknown }).ethereum;
  social.fail = false;
});

describe("the sign-in page", () => {
  it("lays out decentraland.org's live options: the email box, Google, and the rest behind More Options", () => {
    page();
    expect(screen.getByRole("heading", { name: "Log in or Sign up to Jump In" })).toBeTruthy();
    expect(screen.getByPlaceholderText("Enter Your Email")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Continue with Google" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Continue with MetaMask" })).toBeNull();
    expect(screen.queryByTestId("more-options")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "More Options" }));
    const grid = screen.getByTestId("more-options");
    expect(within(grid).getAllByRole("button").map((b) => b.getAttribute("aria-label"))).toEqual([
      "MetaMask",
      "Discord",
      "Apple",
      "X",
      "Fortmatic",
      "Coinbase",
      "WalletConnect",
    ]);
    // Without the extension MetaMask cannot be used, and the tooltip says why.
    expect((within(grid).getByRole("button", { name: "MetaMask" }) as HTMLButtonElement).disabled).toBe(true);
    expect(within(grid).getByText(/install the MetaMask Browser Extension/)).toBeTruthy();
  });

  it("offers MetaMask beside Google once the browser carries it", () => {
    installWallet(() => null);
    page();
    expect(screen.getByRole("button", { name: "Continue with MetaMask" })).toBeTruthy();
  });

  it("signs in with the browser wallet in two steps and returns the reader to where they were", async () => {
    let approve: (sig: string) => void = () => {};
    const calls = installWallet((req) => {
      if (req.method === "eth_accounts") return [];
      if (req.method === "eth_requestAccounts") return [SIGNER];
      if (req.method === "personal_sign") return new Promise<string>((resolve) => (approve = resolve));
      return null;
    });
    page();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Continue with MetaMask" }));
    });
    expect(screen.getByTestId("connection-status").textContent).toBe("Confirm in your digital wallet extension to continue.");
    expect(calls).toEqual(["eth_accounts", "eth_requestAccounts", "personal_sign"]);
    expect(assign).not.toHaveBeenCalled();

    await act(async () => approve(`0x${"11".repeat(65)}`));
    expect(getIdentity()?.signer).toBe(SIGNER);
    expect(assign).toHaveBeenCalledWith("/shop/items?sortBy=newest");
  });

  it("says the signature was not confirmed only once it was asked for, and tries again on request", async () => {
    let refuse = true;
    installWallet((req) => {
      if (req.method === "eth_accounts") return [SIGNER];
      if (req.method === "personal_sign" && refuse) throw Object.assign(new Error("User rejected the request."), { code: 4001 });
      if (req.method === "personal_sign") return `0x${"22".repeat(65)}`;
      return null;
    });
    page();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Continue with MetaMask" }));
    });
    expect(screen.getByTestId("connection-status").textContent).toBe(
      "You did not confirm this action in your digital wallet extension. To continue, please try again.",
    );
    expect(screen.queryByTestId("connection-error-detail")).toBeNull();

    refuse = false;
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Try Again" }));
    });
    expect(assign).toHaveBeenCalledWith("/shop/items?sortBy=newest");
  });

  it("quotes the wallet's own error when the connection itself fails", async () => {
    installWallet((req) => {
      if (req.method === "eth_accounts") return [];
      throw new Error("window.ethereum.enable is not a function");
    });
    page();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Continue with MetaMask" }));
    });
    expect(screen.getByTestId("connection-status").textContent).toBe(
      "We couldn't complete the connection to your wallet. To continue, please try again.",
    );
    expect(screen.getByTestId("connection-error-detail").textContent).toBe("window.ethereum.enable is not a function");
    fireEvent.click(screen.getByRole("button", { name: "close" }));
    expect(screen.queryByTestId("connection-modal")).toBeNull();
  });

  it("signs in by email: the code is sent, six digits verify it, and the reader goes back", async () => {
    page();
    const input = screen.getByPlaceholderText("Enter Your Email");
    const next = screen.getByRole("button", { name: "NEXT" }) as HTMLButtonElement;
    fireEvent.change(input, { target: { value: "jane.doe@" } });
    expect(next.disabled).toBe(true);
    expect(screen.getByText("Enter A Valid Email Address")).toBeTruthy();

    fireEvent.change(input, { target: { value: "jane.doe@example.com" } });
    await act(async () => {
      fireEvent.click(next);
    });
    expect(auth.startEmailSignIn).toHaveBeenCalledWith("jane.doe@example.com");
    const modal = screen.getByTestId("email-login-modal");
    expect(within(modal).getByText("jane.doe@example.com")).toBeTruthy();
    expect(within(modal).getByText(/Resend Code \(1:30\)/)).toBeTruthy();

    await act(async () => {
      fireEvent.paste(screen.getByTestId("otp-input-0"), { clipboardData: { getData: () => "12 34 56" } });
    });
    expect(auth.verifyEmailSignIn).toHaveBeenCalledWith("jane.doe@example.com", "123456");
    expect(assign).toHaveBeenCalledWith("/shop/items?sortBy=newest");
  });

  it("keeps the code modal open with upstream's message when the code does not verify", async () => {
    auth.verifyEmailSignIn.mockResolvedValueOnce(null);
    page();
    fireEvent.change(screen.getByPlaceholderText("Enter Your Email"), { target: { value: "jane.doe@example.com" } });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "NEXT" }));
    });
    for (let i = 0; i < 6; i++) {
      await act(async () => {
        fireEvent.change(screen.getByTestId(`otp-input-${i}`), { target: { value: String(i) } });
      });
    }
    expect(screen.getByRole("alert").textContent).toBe("The code is invalid or expired. Please resend code");
    expect(assign).not.toHaveBeenCalled();
  });

  it("shows the service's reason under the box when the code cannot be sent", async () => {
    auth.startEmailSignIn.mockRejectedValueOnce(new Error("Failed to fetch"));
    page();
    fireEvent.change(screen.getByPlaceholderText("Enter Your Email"), { target: { value: "jane.doe@example.com" } });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "NEXT" }));
    });
    expect(screen.getByRole("alert").textContent).toBe("Unable to connect. Please check your internet connection and try again.");
    expect(screen.queryByTestId("email-login-modal")).toBeNull();
  });

  it("starts social sign-in with a return to the checked path, never an outside one", async () => {
    page("https://evil.example/steal");
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Continue with Google" }));
    });
    expect(assign).toHaveBeenCalledWith(
      `https://social.example/google?redirectUrl=${encodeURIComponent("http://localhost:3000/auth/callback?next=%2F")}`,
    );
  });

  it("says so when social sign-in cannot start, instead of spinning", async () => {
    social.fail = true;
    page();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Continue with Google" }));
    });
    expect(screen.getByTestId("connection-status").textContent).toBe("Something went wrong. Please try again.");
    expect(screen.getByTestId("connection-error-detail").textContent).toBe("Social sign-in is unavailable.");
    expect(assign).not.toHaveBeenCalled();
  });

  it("ignores a wallet that answers after the reader closed its modal", async () => {
    let approve: (sig: string) => void = () => {};
    installWallet((req) => {
      if (req.method === "eth_accounts") return [SIGNER];
      if (req.method === "personal_sign") return new Promise<string>((resolve) => (approve = resolve));
      return null;
    });
    page();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Continue with MetaMask" }));
    });
    fireEvent.click(screen.getByRole("button", { name: "close" }));
    expect(screen.queryByTestId("connection-modal")).toBeNull();

    await act(async () => approve(`0x${"33".repeat(65)}`));
    expect(getIdentity()).toBeNull();
    expect(assign).not.toHaveBeenCalled();
    expect(screen.queryByTestId("connection-modal")).toBeNull();
  });

  it("hydrates the server's markup without a mismatch when the browser carries MetaMask", async () => {
    installWallet(() => null);
    const errors: unknown[] = [];
    const container = document.createElement("div");
    // Rendered as the server would: no wallet, the full layout with MetaMask first.
    const ethereum = (window as { ethereum?: unknown }).ethereum;
    delete (window as { ethereum?: unknown }).ethereum;
    container.innerHTML = renderToString(<LoginPage redirectTo="/shop" layoutVariant={null} />);
    (window as { ethereum?: unknown }).ethereum = ethereum;
    document.body.appendChild(container);
    await act(async () => {
      hydrateRoot(container, <LoginPage redirectTo="/shop" layoutVariant={null} />, {
        onRecoverableError: (err) => errors.push(err),
      });
    });
    expect(errors).toEqual([]);
    // After hydration the page knows the wallet is there.
    expect((within(container).getByRole("button", { name: "Continue with MetaMask" }) as HTMLButtonElement).disabled).toBe(false);
    container.remove();
  });

  it("verifies a code once a corrected digit completes it, from any box", async () => {
    auth.verifyEmailSignIn.mockResolvedValueOnce(null);
    page();
    fireEvent.change(screen.getByPlaceholderText("Enter Your Email"), { target: { value: "jane.doe@example.com" } });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "NEXT" }));
    });
    await act(async () => {
      fireEvent.paste(screen.getByTestId("otp-input-0"), { clipboardData: { getData: () => "123456" } });
    });
    expect(screen.getByRole("alert").textContent).toBe("The code is invalid or expired. Please resend code");

    await act(async () => {
      fireEvent.change(screen.getByTestId("otp-input-1"), { target: { value: "9" } });
    });
    expect(auth.verifyEmailSignIn).toHaveBeenLastCalledWith("jane.doe@example.com", "193456");
    expect(assign).toHaveBeenCalledWith("/shop/items?sortBy=newest");
  });
});
