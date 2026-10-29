import { screen } from "@testing-library/react";

export const pressedPlaceButtons = () =>
  ["Like", "Dislike", "Favorite"].map((name) => screen.getByRole("button", { name }).getAttribute("aria-pressed"));
