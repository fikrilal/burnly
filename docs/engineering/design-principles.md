# Design Principles

Burnly treats complexity as the primary cost of software design. Correct code is
not sufficient when its structure makes future changes difficult or unsafe.

These principles apply to generated code, handwritten changes, design proposals,
and code review.

## Optimize For Lower Complexity

- Prefer the design that leaves fewer concepts, dependencies, states, and rules
  for future contributors to understand.
- Evaluate complexity at the system level, not only by local line count.
- Do not trade a small local simplification for wider coupling or hidden global
  complexity.

## Prefer Deep Modules

- Prefer small, stable interfaces that hide substantial implementation detail.
- A new interface must remove meaningful complexity from its callers.
- Avoid thin wrappers that rename an underlying API without improving the
  abstraction.

## Hide Information

- Keep storage schemas, transport DTOs, collector envelopes, process details, and
  platform APIs behind their owning boundaries.
- Expose domain meaning rather than implementation mechanics.
- Do not make callers coordinate steps that a module can own internally.

## Design Away Special Cases

- Repeated conditionals, mode flags, and source-specific branches are design
  signals.
- Prefer a general invariant or abstraction when it removes real special cases.
- Do not create an abstraction solely to make unlike behavior appear uniform.

## Separate Interface From Implementation

- Public contracts describe what a module provides, not how it provides it.
- Implementation details may change without forcing unrelated callers to change.
- Tests should primarily verify observable contracts and important invariants.

## Apply KISS, YAGNI, And DRY Pragmatically

- KISS: choose the simplest design that preserves the required boundaries and
  invariants.
- YAGNI: do not add extension points, configuration, indirection, or generalized
  behavior without a current requirement.
- DRY: remove duplicated knowledge and policy, not merely similar-looking lines.
- Duplication is cheaper than a premature abstraction that couples unrelated
  behavior.

## Prefer Strategic Design

- Fix the cause of recurring complexity instead of adding tactical exceptions.
- Small tactical patches are acceptable only when their debt is explicit and
  bounded.
- When an area is repeatedly difficult to change, reconsider its interface and
  ownership before adding more code.

## Review Standard

Reject or redesign changes that:

- Add abstractions for hypothetical reuse.
- Expose implementation details across an architectural boundary.
- Add boolean flags that create behavioral modes instead of distinct concepts.
- Add special cases without explaining why a general design is inappropriate.
- Grow a public interface without hiding proportionally greater complexity.
- Reduce duplication by introducing unclear ownership or tighter coupling.

Mechanical checks support these principles, but they do not replace design
judgment. Execution plans and reviews must evaluate the complexity that tools
cannot measure.
