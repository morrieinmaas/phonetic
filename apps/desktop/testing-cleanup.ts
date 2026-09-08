/**
 * Registers a global `afterEach(cleanup)` for `bun test` component tests.
 *
 * `@testing-library/react` normally does this itself as a side effect of
 * import, unmounting whatever the previous test rendered so the next one
 * starts from an empty document. With more than one test file that renders
 * present in the same `bun test` run, that side-effect import only ends up
 * scoped to whichever file's module graph happens to evaluate it first --
 * every *other* render-based file then accumulates un-unmounted trees across
 * its own tests and starts failing on ambiguous "found multiple elements"
 * matches, even though each file passes fine in isolation.
 *
 * Registering cleanup once here instead, before any test file's
 * `describe`/`it` tree exists, applies it globally regardless of import
 * order or how many files render. This must load as its own preload entry
 * *after* `happydom.ts` (see bunfig.toml) rather than being folded into that
 * file: `@testing-library/react` needs `document` on `globalThis` at import
 * time, and ES module imports are hoisted above a file's own top-level code
 * no matter where the `import` statement sits textually, so importing it
 * from within `happydom.ts` itself would run before that file's own global
 * registration.
 */
import { afterEach } from "bun:test";
import { cleanup } from "@testing-library/react";

afterEach(() => {
	cleanup();
});
