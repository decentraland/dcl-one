import { expect, test, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import CommunityMemberActions from "./CommunityMemberActions";

const viewer = "0x1111111111111111111111111111111111111111";
const member = { memberAddress: "0x2222222222222222222222222222222222222222", name: "Member B", role: "member" };

test.each([
  ["owner", "member", "Make moderator"], ["owner", "moderator", "Make member"], ["owner", "mod", "Make member"],
  ["moderator", "member", null], ["mod", "member", null],
])("%s actions on %s match the role boundary", async (role, target, assignment) => {
  const user = userEvent.setup();
  render(<CommunityMemberActions member={{ ...member, role: target }} viewer={viewer} role={role} write={vi.fn()} />);
  await user.click(screen.getByRole("button", { name: "Member actions" }));
  expect(screen.getByRole("menuitem", { name: "Remove member" })).toBeInTheDocument();
  expect(screen.queryByRole("menuitem", { name: /^Make / })?.textContent ?? null).toBe(assignment);
});

test.each([
  ["owner", "owner"], ["moderator", "owner"], ["moderator", "moderator"], ["mod", "mod"],
  ["member", "member"], ["admin", "member"], ["owner", "admin"], ["owner", "banned"],
])("%s cannot manage a %s through these controls", (role, target) => {
  render(<CommunityMemberActions member={{ ...member, role: target }} viewer={viewer} role={role} write={vi.fn()} />);
  expect(screen.queryByRole("button", { name: "Member actions" })).toBeNull();
});

test("self-management and anonymous management have no controls", () => {
  const { rerender } = render(<CommunityMemberActions member={{ ...member, memberAddress: viewer }} viewer={viewer} role="owner" write={vi.fn()} />);
  expect(screen.queryByRole("button")).toBeNull();
  rerender(<CommunityMemberActions member={member} viewer={null} role="owner" write={vi.fn()} />);
  expect(screen.queryByRole("button")).toBeNull();
});

test("confirmation submits once, preserves a refusal, and cancellation never changes membership", async () => {
  const user = userEvent.setup();
  let reject!: (error: Error) => void;
  const write = vi.fn(() => new Promise<void>((_, fail) => { reject = fail; }));
  render(<CommunityMemberActions member={member} viewer={viewer} role="moderator" write={write} />);
  await user.click(screen.getByRole("button", { name: "Member actions" }));
  await user.click(screen.getByRole("menuitem", { name: "Remove member" }));
  expect(write).not.toHaveBeenCalled();
  await user.dblClick(screen.getByRole("button", { name: "Remove member" }));
  expect(write).toHaveBeenCalledExactlyOnceWith("DELETE", member.memberAddress, undefined);
  await user.click(screen.getByRole("button", { name: "Cancel" }));
  expect(screen.getByRole("alertdialog")).toBeInTheDocument();
  await act(async () => reject(Object.assign(new Error("The user 0x… cannot kick 0x… from community …"), { status: 401 })));
  expect(screen.getByRole("alert")).toHaveTextContent("You don't have permission to change this member.");
  await user.click(screen.getByRole("button", { name: "Cancel" }));
  expect(screen.queryByRole("alertdialog")).toBeNull();
  expect(write).toHaveBeenCalledTimes(1);
});

test("role confirmation preserves the member identity and uses the requested role", async () => {
  const user = userEvent.setup(), write = vi.fn(async () => {});
  render(<CommunityMemberActions member={member} viewer={viewer} role="owner" write={write} />);
  await user.click(screen.getByRole("button", { name: "Member actions" }));
  await user.click(screen.getByRole("menuitem", { name: "Make moderator" }));
  await user.click(screen.getByRole("button", { name: "Save role" }));
  expect(write).toHaveBeenCalledExactlyOnceWith("PATCH", member.memberAddress, { role: "moderator" });
  expect(screen.queryByRole("alertdialog")).toBeNull();
});
