// @wdio/mocha-framework runs specs through Mocha's BDD interface and injects
// `describe`/`it` as globals — it does not ship type declarations for them
// (only `@types/mocha` does, and that package isn't part of this project's
// resolvable dependency tree). The BDD interface is small and has been
// stable for years, so it's hand-declared here rather than pulling in an
// extra dependency just for two function signatures.
declare function describe(name: string, fn: () => void): void;
declare function it(name: string, fn: () => Promise<void> | void): void;
declare function before(fn: () => Promise<void> | void): void;
declare function after(fn: () => Promise<void> | void): void;
