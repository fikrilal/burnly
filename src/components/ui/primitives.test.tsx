import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Badge } from "./badge";
import { Separator } from "./separator";
import { Skeleton } from "./skeleton";

describe("Badge", () => {
  it("renders children", () => {
    render(<Badge>Stable</Badge>);
    expect(screen.getByText("Stable")).toBeInTheDocument();
  });

  it("applies the destructive variant styling", () => {
    render(<Badge variant="destructive">Failed</Badge>);
    expect(screen.getByText("Failed").className).toContain("text-destructive");
  });
});

describe("Separator", () => {
  it("renders a decorative horizontal divider by default", () => {
    const { container } = render(<Separator />);
    const separator = container.querySelector('[data-slot="separator"]');
    expect(separator).not.toBeNull();
    expect(separator?.className).toContain("h-px");
  });

  it("exposes a separator role when not decorative", () => {
    render(<Separator decorative={false} orientation="vertical" />);
    expect(screen.getByRole("separator")).toHaveAttribute(
      "aria-orientation",
      "vertical",
    );
  });
});

describe("Skeleton", () => {
  it("renders a token-based placeholder", () => {
    const { container } = render(<Skeleton className="h-4 w-10" />);
    const skeleton = container.querySelector('[data-slot="skeleton"]');
    expect(skeleton).not.toBeNull();
    expect(skeleton?.className).toContain("bg-muted");
  });
});
