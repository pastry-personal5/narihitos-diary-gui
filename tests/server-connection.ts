import { fireEvent, screen } from "@testing-library/react";

export function connectFromSettings() {
  if (!screen.queryByRole("navigation", { name: "Settings sections" })) {
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
  }
  fireEvent.click(screen.getByRole("button", { name: "Server", exact: true }));
  fireEvent.click(screen.getByRole("button", { name: "Connect" }));
}
