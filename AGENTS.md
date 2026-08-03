# Herbie — Agent Notes

## Commands

- `pnpm dev` — run the app in development.
- `pnpm build` — type-check + build main/preload/renderer to `out/`.
- `pnpm typecheck` — type-check main/preload (`tsc`) and renderer (`vue-tsc`); no emit.
- `pnpm lint` — ESLint over `src/` and `tests/` (flat config, `eslint.config.js`).
- `pnpm test` — run Vitest unit + integration tests once. Tests run **under Electron as
  Node** (`ELECTRON_RUN_AS_NODE=1 electron ...vitest`) so the Electron-built
  `better-sqlite3` native module (ABI for Electron's Node) loads. Running plain
  `vitest` under system Node fails on the native module ABI — use `pnpm test`.
- `pnpm test:watch` — Vitest watch mode (also under Electron).
- `pnpm rebuild` — rebuild `better-sqlite3` native module against the installed Electron.
- `pnpm build:installer` — build + package installer via electron-builder.

## Architecture

- Source of truth for behavior: `docs/milestone-1-requirements.md` and the implementation
  plan. `CONTEXT.md` defines the domain glossary (Todo / Label / Quick Add).
- Electron main process owns the `better-sqlite3` singleton and all persistence logic.
  Renderer never touches Node. `contextIsolation: true`, `sandbox: true`,
  `nodeIntegration: false`.
- `src/main/db-access.ts` is an **Electron-free** holder of the DB singleton so the repos
  (`todos`, `settings`, `labels-store`, `export`) can be unit-tested in pure Node via an
  in-memory `new Database(':memory:')` + `runMigrations`. `src/main/db.ts` is the only
  Electron-bound module (`app.getPath`) and seeds the singleton via `setDb` on ready.
- Shared pure code (`src/shared/`) is imported by both main and renderer (labels, time,
  markdown). Keep these dependency-free and unit-tested.
- Migrations live in `migrations/` as ordered SQL files (embedded via `?raw`); applied at
  startup by the migrations runner in `src/main/migrations.ts`, which accepts a `Database`
  so tests can call it directly.

## Deviation from requirements

`milestone-1-requirements.md` §5 asks for auto-export on startup. The user overrode this to
**manual-only export**. Do NOT add an export call in `app.whenReady`. The single
attach point to re-enable auto-export is in `src/main/index.ts` (see comment).

## Milestone 2 — time tracking

- **Native module `herbie-winhook`** is an in-tree N-API module under
  `native/herbie-winhook/` (Windows-only, C++ + `node-addon-api`). It is wired as a local
  file dependency in `package.json` and listed in `pnpm.onlyBuiltDependencies`. Building it
  requires **Visual Studio Build Tools (C++) + Python** and is compiled by
  `electron-rebuild` during `postinstall` / `pnpm rebuild`. Required native libs:
  `user32`, `psapi`, `kernel32` (declared in `binding.gyp`).
- If the native module is absent or fails to build, `loadWinHook()` in `src/main/index.ts`
  degrades to a no-op notifier and the app keeps running **without segment recording** —
  this is intentional for cross-platform dev.
- **Tray residency change**: milestone 2 makes the app stay in the tray when the last
  window is closed (`window-all-closed` now hides the main window instead of quitting on
  Windows). Quit only happens via the tray 退出 item (`before-quit` tears down tracking +
  closes the DB). This is a behavior change from milestone 1.
- Segment tracking lives in `src/main/segments.ts` (state machine) + `src/main/tracker.ts`
  (idle poll + off-work, **dependency-injected** so it stays unit-testable). Aggregation is
  pure in `src/shared/segments-agg.ts`; day slicing is pure in `src/shared/time.ts`
  (`splitAtMidnight`). `powerMonitor` must only be touched inside `app.whenReady`.