import { describe, expect, test } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";

import AvatarFace from "./AvatarFace";

const profile = (face256?: string) => ({ name: "Ada", avatar: { snapshots: { face256 } } });

describe("AvatarFace", () => {
  test("the face is an image named after the profile", () => {
    render(<AvatarFace avatar={profile("https://content.example/face.png")} />);

    expect(screen.getByRole("img", { name: "Ada" })).toHaveAttribute("src", "https://content.example/face.png");
  });

  test("a snapshot that is a content hash loads from the content URL", () => {
    render(<AvatarFace avatar={profile("bafyface")} contentUrl="https://content.example/contents/" />);

    expect(screen.getByRole("img", { name: "Ada" })).toHaveAttribute("src", "https://content.example/contents/bafyface");
  });

  test("a profile with no face shows the guest glyph with the name", () => {
    render(<AvatarFace avatar={{ name: "Ada" }} />);

    const face = screen.getByRole("img", { name: "Ada" });
    expect(face.tagName).toBe("SPAN");
    expect(face.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });

  test("an image that fails to load falls back to the guest glyph", () => {
    render(<AvatarFace avatar={profile("https://content.example/missing.png")} />);

    fireEvent.error(screen.getByRole("img", { name: "Ada" }));

    const face = screen.getByRole("img", { name: "Ada" });
    expect(face.tagName).toBe("SPAN");
    expect(document.querySelector("img")).toBeNull();
  });

  test("an empty alt makes the face decorative", () => {
    const { container, rerender } = render(<AvatarFace avatar={profile("https://content.example/face.png")} alt="" />);

    expect(screen.queryByRole("img")).toBeNull();
    expect(container.querySelector("img")).toHaveAttribute("alt", "");

    rerender(<AvatarFace avatar={null} alt="" />);
    expect(screen.queryByRole("img")).toBeNull();
  });

  test("size, inline and a class of the caller are set on the root", () => {
    const { container } = render(<AvatarFace avatar={null} size="tiny" inline className="mine" />);

    const root = container.firstElementChild;
    expect(root).toHaveAttribute("data-size", "tiny");
    expect(root).toHaveAttribute("data-inline");
    expect(root).toHaveClass("mine");
  });
});
