# Execution Plans

Non-trivial implementation work uses an execution plan in:

```text
docs/exec-plans/active/
```

Only plans currently being implemented belong in `active/`. Keep one active
implementation chunk by default. A phase overview may remain active beside that
chunk when it coordinates several dependent plans.

Approved future plans that are not ready to implement belong in:

```text
docs/exec-plans/queued/
```

Queued plans define scope and dependencies, but their implementation details may
be refined when they become active. Do not mark queued checklist items complete
or make product changes from a queued plan.

Completed plans move to:

```text
docs/exec-plans/completed/
```

Use `_template.md` as the starting point.

## Multi-Chunk Phases

For a phase split into multiple implementation chunks:

1. Keep one phase overview in `active/`.
2. Keep the current chunk in `active/`.
3. Keep dependent future chunks in `queued/`.
4. Complete and verify the current chunk.
5. Move the completed chunk to `completed/`.
6. Update progress and decisions in the phase overview.
7. Move the next unblocked queued plan to `active/`.
8. Move the phase overview to `completed/` only after all phase exit criteria pass.

The repository is the durable memory for scope, dependencies, decisions, and
verification outcomes. Conversation history is not the source of truth.
