import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import Passport from "./Passport";
import { sendBridge } from "../../overlay/bridge";
vi.mock("../../overlay/bridge", () => ({ sendBridge: vi.fn(), getBridge: () => null }));

test("only the own profile offers avatar editing", () => {
  const own = render(<Passport isSelf identity={{ name: "Brown" }} />);
  expect(screen.getByRole("button", { name: /edit avatar/i })).toBeInTheDocument();
  own.unmount();
  render(<Passport isSelf={false} identity={{ name: "Lu" }} />);
  expect(screen.queryByRole("button", { name: /edit avatar/i })).toBeNull();
});

test("name editing stays open on blur and saves explicitly through the bridge", () => {
  render(<Passport identity={{ name: "Brown" }} base={{ bodyShape: "body", skinColor: "#123456" }} />);
  fireEvent.click(screen.getByRole("button", { name: "Edit name" }));
  const input = screen.getByRole("textbox", { name: "Edit name" });
  fireEvent.change(input, { target: { value: "NewName" } });
  fireEvent.blur(input);
  expect(input).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Save name" }));
  expect(sendBridge).toHaveBeenCalledWith("SetAvatar", expect.objectContaining({ base: expect.objectContaining({ name: "NewName", bodyShapeUrn: "body" }) }));
  expect(screen.queryByRole("textbox")).toBeNull();
});

test("overview excludes badges and the badges tab excludes overview sections", () => {
  render(<Passport badges={[{id:"badge",name:"Explorer"}]} about="About this explorer" />);
  expect(screen.getByText("About this explorer")).toBeInTheDocument();
  expect(screen.queryByRole("heading", { name: "Badges" })).toBeNull();
  fireEvent.click(screen.getByRole("tab", { name: "Badges" }));
  expect(screen.getByRole("heading", { name: "Badges" })).toBeInTheDocument();
  expect(screen.queryByText("About this explorer")).toBeNull();
});

test("dismisses only a backdrop click and respects the selected badges tab", () => {
  const onClose = vi.fn();
  const { container } = render(<Passport tab="badges" onClose={onClose} />);
  expect(screen.getByRole("tab", { name: "Badges" })).toHaveAttribute("aria-selected", "true");
  fireEvent.click(container.querySelector(".ps")!);
  expect(onClose).not.toHaveBeenCalled();
  fireEvent.click(container.querySelector(".ps__backdrop")!);
  expect(onClose).toHaveBeenCalledTimes(1);
});

test("another explorer's passport sends a friend request only after confirmation", async () => {
  const onAddFriend = vi.fn(() => Promise.resolve());
  const view = render(<Passport isSelf={false} identity={{ name: "Lu" }} onAddFriend={onAddFriend} />);
  fireEvent.click(screen.getByRole("button", { name: "Add friend" }));
  expect(onAddFriend).not.toHaveBeenCalled();
  expect(screen.getByRole("dialog", { name: "Send friend request" })).toHaveTextContent("Send Lu a friend request?");
  fireEvent.click(screen.getByRole("button", { name: "Send request" }));
  expect(onAddFriend).toHaveBeenCalledTimes(1);
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "Send friend request" })).toBeNull());
  view.rerender(<Passport isSelf={false} identity={{ name: "Lu" }} friendship="requested" onAddFriend={onAddFriend} />);
  expect(screen.getByText("Requested")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Add friend" })).toBeNull();
  view.unmount();
  render(<Passport isSelf identity={{ name: "Brown" }} onAddFriend={onAddFriend} />);
  expect(screen.queryByRole("button", { name: "Add friend" })).toBeNull();
});

test("a failed friend request keeps the confirmation open with the reason", async () => {
  render(<Passport isSelf={false} identity={{ name: "Lu" }} onAddFriend={() => Promise.reject(new Error("No reply from the server. Try again."))} />);
  fireEvent.click(screen.getByRole("button", { name: "Add friend" }));
  fireEvent.click(screen.getByRole("button", { name: "Send request" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("No reply from the server. Try again.");
  expect(screen.getByRole("dialog", { name: "Send friend request" })).toBeInTheDocument();
});

test("a pending request from that explorer offers accept, and a blocked one offers no request", async () => {
  const onAddFriend = vi.fn(() => Promise.resolve());
  const onAcceptFriend = vi.fn(() => Promise.resolve());
  const view = render(<Passport isSelf={false} identity={{ name: "Lu" }} friendship="received" onAddFriend={onAddFriend} onAcceptFriend={onAcceptFriend} />);
  expect(screen.queryByRole("button", { name: "Add friend" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Accept friend request" }));
  await waitFor(() => expect(onAcceptFriend).toHaveBeenCalledTimes(1));
  expect(onAddFriend).not.toHaveBeenCalled();
  view.rerender(<Passport isSelf={false} identity={{ name: "Lu" }} friendship="blocked" onAddFriend={onAddFriend} onAcceptFriend={onAcceptFriend} />);
  expect(screen.getByText("Blocked")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /Add friend|Accept/ })).toBeNull();
});
