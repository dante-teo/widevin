# Architecture

## Overview

Widevin is planned as a TypeScript library package. The architecture should keep
runtime behavior small, explicit, and easy to test. Public APIs should be
implemented as composable functions wherever possible, with side effects isolated
at the package boundary.

## Design Principles

- Prefer pure functions over classes and shared mutable state.
- Keep public exports deliberate and minimal.
- Make invalid states difficult to represent with TypeScript types.
- Separate core logic from I/O, environment access, and package tooling.
- Test behavior through public APIs where practical.
- Use open source libraries when they solve a real problem better than custom
  code.

## Suggested Project Layout

```text
.
├── docs/
│   ├── ARCHITECTURE.md
│   └── PRODUCT.md
├── src/
│   ├── index.ts
│   └── ...
├── test/
│   └── ...
├── package.json
├── tsconfig.json
└── README.md
```

## Source Organization

Use `src/index.ts` as the public entry point. Keep implementation modules
private by default and export only stable symbols from the entry point.

Prefer this shape:

```ts
export { createThing } from "./create-thing";
export type { Thing, ThingOptions } from "./types";
```

Avoid broad wildcard exports unless the module is explicitly designed as part of
the public API.

## API Style

Public APIs should generally be:

- Function-based.
- Immutable in inputs and outputs.
- Explicit about errors.
- Easy to tree-shake.
- Friendly to both TypeScript and JavaScript consumers.

Prefer returning values instead of mutating parameters. If a function can fail in
an expected way, model that failure intentionally rather than relying on
incidental exceptions.

## Build Outputs

The package should eventually publish compiled JavaScript and TypeScript
declarations. A typical npm-ready setup should define package exports explicitly,
for example:

```json
{
  "type": "module",
  "exports": {
    ".": {
      "types": "./dist/index.d.ts",
      "default": "./dist/index.js"
    }
  },
  "files": ["dist", "README.md", "LICENSE"]
}
```

The exact module format should be chosen when the package tooling is added.

## Testing Strategy

Follow test-driven development for all implementation work:

1. Write a failing test that describes the desired behavior.
2. Implement the smallest change that passes the test.
3. Refactor while keeping tests green.

Prioritize unit tests for pure functions. Add integration tests only when
behavior crosses package boundaries such as filesystem access, process
environment access, package exports, or generated build artifacts.

## Dependency Policy

Prefer well-maintained open source packages for established problems such as
schema validation, parsing, formatting, or protocol handling. Before adding a
dependency, check:

- Maintenance activity.
- License compatibility.
- Bundle and install cost.
- Whether it belongs in `dependencies` or `devDependencies`.
- Whether the same result can be achieved safely with platform APIs.

## Publishing Architecture

Before npm publication, the package should have:

- A reproducible build command.
- A clean `dist` output.
- Explicit package exports.
- Generated declaration files.
- A prepublish or release check that runs tests and type checks.
- No accidental source, test fixture, or local configuration files in the
  published package.

## Future Decisions

These decisions should be made once implementation requirements are clearer:

- Test runner.
- Build tool.
- Module target and runtime support policy.
- Whether to support CommonJS consumers.
- Runtime dependency list.
- Release automation.
