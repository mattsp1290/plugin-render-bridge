# Scaffold publication audit

This main-branch foundation contains package metadata, MIT LICENSE, a neutral
README, CI definitions, an empty library and an empty renderer target. It
contains no imported source code, plugin fixture, preset or rendered audio.
Implementation belongs to `feat/plugin-render-bridge-plan-kf32` and has its
own completed publication audit and approval gate.

L3: no source files were imported. The initial commit records the intended
extraction source revision for provenance; source history was not copied.
L4: scaffold README does not use the format trademark.
L5: package inspection lists only Cargo metadata/lockfile, LICENSE, README,
this audit and the two scaffold source files. There are no personal paths,
credentials, service identifiers or application dependencies in code.
Normal dependencies contain no ms-*, lotel-*, Tauri or Specta package, and there
is one hosting dependency pinned with the plan's exact git/tag spelling.
Cargo.lock is identical to the implementation lockfile. The 97-package
`cargo license --json` report offers permissive MIT, Apache-2.0, ISC or
Unlicense options throughout; Unicode-3.0 also applies to unicode-ident.
No copyleft-only dependency is included. The original scaffold builds locally.

No imported implementation is placed on main by this bootstrap. Remote CI on
this scaffold and explicit implementation publication approval remain pending.
