import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { Dialog, DialogContent, DialogTitle, DialogTrigger } from "./dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "./dropdown-menu";
import { Popover, PopoverContent, PopoverTrigger } from "./popover";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "./tooltip";

describe("Dialog", () => {
  it("opens from its trigger and exposes a titled dialog", async () => {
    const user = userEvent.setup();
    render(
      <Dialog>
        <DialogTrigger>Open</DialogTrigger>
        <DialogContent>
          <DialogTitle>Confirm</DialogTitle>
        </DialogContent>
      </Dialog>,
    );

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Open" }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("Confirm")).toBeInTheDocument();
  });
});

describe("DropdownMenu", () => {
  it("opens and shows its items", async () => {
    const user = userEvent.setup();
    render(
      <DropdownMenu>
        <DropdownMenuTrigger>Menu</DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuItem>Refresh now</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>,
    );

    await user.click(screen.getByRole("button", { name: "Menu" }));
    expect(
      screen.getByRole("menuitem", { name: "Refresh now" }),
    ).toBeInTheDocument();
  });
});

describe("Popover", () => {
  it("opens and shows its content", async () => {
    const user = userEvent.setup();
    render(
      <Popover>
        <PopoverTrigger>Details</PopoverTrigger>
        <PopoverContent>Popover body</PopoverContent>
      </Popover>,
    );

    await user.click(screen.getByRole("button", { name: "Details" }));
    expect(screen.getByText("Popover body")).toBeInTheDocument();
  });
});

describe("Tooltip", () => {
  it("shows content on hover", async () => {
    const user = userEvent.setup();
    render(
      <TooltipProvider delayDuration={0}>
        <Tooltip>
          <TooltipTrigger>Hover me</TooltipTrigger>
          <TooltipContent>Freshness</TooltipContent>
        </Tooltip>
      </TooltipProvider>,
    );

    await user.hover(screen.getByRole("button", { name: "Hover me" }));
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Freshness");
  });
});
