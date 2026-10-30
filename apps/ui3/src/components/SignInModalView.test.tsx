import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import SignInModalView from "./SignInModalView";

const props = {
  onClose: vi.fn(), inAppAvailable: false, walletAvailable: false, desktopShell: false, authError: null,
  onStartEmailSignIn: vi.fn(), onVerifyEmailSignIn: vi.fn(), onSocialSignIn: vi.fn(),
  onConnectWallet: vi.fn(), onBeginShellSignIn: () => ({ identity: Promise.resolve(null), cancel: vi.fn() }),
};
afterEach(() => { cleanup(); vi.clearAllMocks(); });
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

it("cancels an in-flight startup when the user goes back", async () => {
  const connected = deferred<unknown>();
  const session = { uri: "wc:example@2", qrDataUrl: "data:image/svg+xml,", connected: connected.promise, cancel: vi.fn() };
  const starting = deferred<typeof session>();
  const onWalletConnect = vi.fn((_signal: AbortSignal) => starting.promise);
  render(<SignInModalView {...props} onWalletConnect={onWalletConnect} />);
  fireEvent.click(screen.getByRole("button", { name: "Continue with wallet" }));
  fireEvent.click(screen.getByRole("button", { name: /Back/ }));
  expect(onWalletConnect.mock.calls[0]![0].aborted).toBe(true);
  await act(async () => { starting.resolve(session); connected.resolve({ signer: "late" }); });
  expect(session.cancel).toHaveBeenCalled();
  expect(props.onClose).not.toHaveBeenCalled();
  expect(screen.queryByAltText("WalletConnect QR code")).toBeNull();
});

it("shows wallet rejection and cancels the session when closed", async () => {
  const connected = deferred<unknown>();
  const cancel = vi.fn();
  const view = render(<SignInModalView {...props} onWalletConnect={async () => ({
    uri: "wc:example@2", qrDataUrl: "data:image/svg+xml,", connected: connected.promise, cancel,
  })} />);
  fireEvent.click(screen.getByRole("button", { name: "Continue with wallet" }));
  await screen.findByAltText("WalletConnect QR code");
  await act(async () => connected.reject(new Error("Connection rejected by wallet")));
  expect(screen.getByRole("alert").textContent).toBe("Connection rejected by wallet");
  expect(props.onClose).not.toHaveBeenCalled();
  view.unmount();
  expect(cancel).toHaveBeenCalled();
});

it("omits QR sign-in when the deployment has no WalletConnect configuration", () => {
  render(<SignInModalView {...props} walletAvailable />);
  fireEvent.click(screen.getByRole("button", { name: "Continue with wallet" }));
  expect(screen.getByRole("button", { name: "Use a browser wallet" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Continue with WalletConnect" })).toBeNull();
});
