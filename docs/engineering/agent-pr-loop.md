# Agent PR Loop

Use this loop for implementation work:

1. Read the relevant approved docs.
2. Create or update an execution plan for non-trivial work.
3. Define the test plan at the lowest stable layer.
4. Make the smallest coherent change.
5. Run targeted checks while iterating.
6. Run `pnpm verify` before completion when feasible.
7. Record verification outcomes in the execution plan.
8. Move completed plans to `docs/exec-plans/completed/`.

Do not treat code compilation as the only definition of done.
