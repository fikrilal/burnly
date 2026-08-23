# Burnly Technology Stack

## Purpose

This document records the technologies selected for Burnly.

It does not define application architecture, data flow, repository structure, module boundaries, or deployment design. Those decisions belong in separate documents.

## Supported Platforms

Burnly will support:

- macOS
- Windows
- Linux

## Desktop Application

### Tauri 2

Tauri 2 will provide the cross-platform desktop runtime.

It was selected because it supports Burnly's target operating systems, system-tray applications, native capabilities, application packaging, and updates while keeping the application relatively lightweight.

## User Interface

### React

React will be used to build the desktop user interface.

It provides a mature component ecosystem and can support future optional web
surfaces, such as sync or leaderboard views, without requiring the product to
adopt a separate UI paradigm prematurely.

### TypeScript

TypeScript will be the primary language for user-interface code.

It provides static type checking and makes shared data contracts easier to maintain.

### Vite

Vite will provide the frontend development and build tooling.

It is a focused choice for a client-side Tauri application and avoids server-oriented features that Burnly does not currently need.

### Tailwind CSS

Tailwind CSS will provide styling utilities.

It supports consistent visual tokens and fast interface development while allowing Burnly to retain full control over its visual identity.

### Radix UI

Radix UI will provide accessible, unstyled interface primitives where appropriate.

It will be used selectively for behavior-heavy controls such as dialogs, menus, tooltips, popovers, tabs, and switches.

### Lucide

Lucide will provide interface icons.

It offers a consistent icon set suitable for desktop tools and common application actions.

## Data Visualization

### Apache ECharts

Apache ECharts will provide interactive charts and data visualizations.

It supports the small trends, comparisons, and tooltips needed by Burnly's tray
tracker and secondary detail views.

The activity calendar may use a dedicated React component or a custom implementation if that produces a clearer and more accessible result than a general-purpose chart.

## Frontend State and Data

### Zustand

Zustand will manage client-side interface state.

It should remain limited to state that is genuinely shared across the interface.

### TanStack Query

TanStack Query will manage asynchronous data loading, caching, refresh state, and errors in the user interface.

### Zod

Zod will validate data entering the TypeScript layer.

It provides runtime validation in addition to TypeScript's compile-time checks.

## Native Application Code

### Rust

Rust will be used for native desktop capabilities and system-level work.

It is the native language used by Tauri and is suitable for long-running local operations where reliability and resource efficiency matter.

## Local Database

### SQLite

SQLite will store Burnly's local application data.

It is embedded, durable, widely supported, and appropriate for a local-first desktop application.

### rusqlite

The Rust application will access SQLite through `rusqlite`.

It provides direct SQLite integration without requiring a separate database service.

## Usage Data Source

### Collectors

Burnly uses replaceable Rust infrastructure adapters behind a Burnly-owned
collector contract. A pinned bundled `ccusage` sidecar collects Claude Code,
Codex, and Pi. Sources whose local formats require stronger completeness or
privacy controls may use native read-only Rust collectors; OpenCode uses this
path for its legacy and preview V2 SQLite schemas.

Burnly will not require users to install `ccusage` separately.

Neither collector path exposes its storage or envelope types to application or
domain code.

## Testing

### Vitest

Vitest will run TypeScript unit and integration tests.

### React Testing Library

React Testing Library will test user-interface behavior through user-visible interactions.

### Cargo Test

Rust's built-in test tooling will test native application code.

### Playwright

Playwright will provide end-to-end testing for critical user workflows and visual verification.

## Code Quality

### ESLint

ESLint will enforce TypeScript and React code-quality rules.

### Prettier

Prettier will provide consistent formatting for supported frontend and documentation files.

### rustfmt

`rustfmt` will format Rust code.

### Clippy

Clippy will provide additional correctness and quality checks for Rust code.

## Package Management

### pnpm

pnpm will manage JavaScript and TypeScript dependencies.

### Cargo

Cargo will manage Rust dependencies and builds.

## Continuous Integration and Releases

### GitHub Actions

GitHub Actions will run automated checks and cross-platform release workflows.

### Tauri Action

Tauri Action will build platform-specific desktop release artifacts through GitHub Actions.

## Explicitly Not Selected

### Electron

Electron is not selected because Burnly is intended to run continuously as a lightweight desktop and system-tray application. Its mature ecosystem does not currently outweigh its larger runtime and resource footprint for this product.

### Next.js

Next.js is not selected for the desktop application because Burnly does not need server rendering or a server-oriented application framework.

The future web product can choose its own framework when its requirements are
defined.

### Flutter

Flutter is not selected because React and TypeScript better align with the planned desktop interface and potential future web reuse, while Tauri provides the native desktop capabilities Burnly needs.

### A Remote Database

A hosted database is not part of the local desktop stack. Remote storage will be
evaluated separately when optional account synchronization and web/social
features are designed.

## Decision Status

These choices are approved for the initial Burnly desktop application.

They may be revisited when product requirements reveal a concrete limitation. Changes should be based on measured needs rather than speculative future requirements.
