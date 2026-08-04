// Cross-platform install-safe rebuild.
//   node scripts/rebuild.js          -> rebuild better-sqlite3 always; rebuild
//                                      herbie-winhook only on Windows (Windows-only native
//                                      module that needs VS Build Tools). On non-Windows
//                                      it is skipped and index.js degrades to a no-op at
//                                      runtime, keeping `pnpm install` green (AGENTS.md).
//   node scripts/rebuild.js --native -> rebuild herbie-winhook only (Windows).
// Note: @electron/rebuild@4 split `-w/--which-module` (non-exclusive) from
// `-o/--only` (exclusive). We use `-o` so rebuilding better-sqlite3 does NOT also
// pull in the Windows-only herbie-winhook (which would fail with `windows.h` on
// Linux and with `napi.h` when its node-addon-api include dir is not resolvable).
const { spawnSync } = require('node:child_process')

function run(args) {
  const res = spawnSync('electron-rebuild', ['-f', ...args], { stdio: 'inherit', shell: true })
  return res.status
}

const onlyNative = process.argv.includes('--native')
const nativeTargets = process.platform === 'win32' ? ['herbie-winhook'] : []

let failed = false

if (!onlyNative) {
  const code = run(['-o', 'better-sqlite3'])
  if (code !== 0) {
    console.error(`electron-rebuild better-sqlite3 failed (exit ${code})`)
    failed = true
  }
}

for (const mod of nativeTargets) {
  const code = run(['-o', mod])
  if (code !== 0) {
    console.error(`electron-rebuild ${mod} failed (exit ${code})`)
    // A missing VS Build Tools toolchain for herbie-winhook should not break install on
    // Windows either — the app runs without segment recording in that case.
    console.error(`(continuing; ${mod} not required to run the app)`)
  }
}

if (failed) process.exit(failed ? 1 : 0)