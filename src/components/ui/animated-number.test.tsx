import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { installMatchMedia } from "@/test/match-media";
import { AnimatedNumber } from "./animated-number";

beforeEach(() => {
  // matches=true => prefers-reduced-motion is on => value renders instantly.
  installMatchMedia(true);
});

describe("AnimatedNumber", () => {
  it("renders the formatted target value immediately under reduced motion", () => {
    render(<AnimatedNumber value={42180} />);
    expect(screen.getByText("42,180")).toBeInTheDocument();
  });

  it("applies a custom formatter", () => {
    render(
      <AnimatedNumber value={1234} format={(n) => `${Math.round(n)} tok`} />,
    );
    expect(screen.getByText("1234 tok")).toBeInTheDocument();
  });
});
