# Releasing

Widevin publishes one shared version to npm and crates.io. A release tag must
use `vX.Y.Z` and match both `package.json` and `rust/Cargo.toml`.

## Bump the Shared Version

Use the root `pnpm bump` script to update the npm manifest, Cargo manifest and
lockfile, and current-version README references together:

```sh
pnpm bump patch
pnpm bump minor
pnpm bump major
pnpm bump prerelease --preid beta
pnpm bump 1.0.0
```

The command first rejects existing version drift, accepts the standard
semantic-version bump types or an explicit version, and then updates all
shared-version files. It requires a clean Git working tree, creates a
version-only commit (for example, `0.1.5`) and annotated tag (`v0.1.5`), and
never pushes or publishes. If writing, committing, or tagging fails, it rolls
the version files and local release commit back. Run the preflight checks below
before bumping, then inspect the local result with `git show --stat` and
`git tag --list`.

The version-file matching supports both LF and CRLF checkouts and preserves
the existing `Cargo.lock` line endings.

## Preflight

From the repository root:

```sh
pnpm install --frozen-lockfile
pnpm run check:versions
pnpm run typecheck
pnpm test
pnpm run build
pnpm pack --pack-destination /tmp

cargo fmt --manifest-path rust/Cargo.toml --check
cargo clippy --manifest-path rust/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path rust/Cargo.toml --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path rust/Cargo.toml --no-deps
cargo package --manifest-path rust/Cargo.toml
```

Inspect both archives before publishing. The npm archive must contain
`LICENSE` and `NOTICE`; the Rust archive must contain its `LICENSE`, `NOTICE`,
and `src/proto.rs`.

## Initial Rust Release

Rust `0.1.4` was published manually because crates.io requires the first
release before trusted publishing can be configured. The command used was:

```sh
cargo publish --manifest-path rust/Cargo.toml
```

Configure the crates.io trusted publisher with:

- Repository: `dante-teo/widevin`
- Workflow: `publish.yml`
- Environment: `release`

## Subsequent Releases

After `pnpm bump` creates the local version commit and matching `vX.Y.Z` tag,
push the commit and tag only with explicit authorization:

```sh
git push origin HEAD
git push origin vX.Y.Z
```

Replace `vX.Y.Z` with the tag created by `pnpm bump`. Pushing the commit alone
does not publish; pushing the matching tag starts the release workflow.

The publish workflow:

1. Verifies TypeScript and Rust.
2. Rejects a tag that differs from either manifest.
3. Publishes crates.io first, skipping an existing version.
4. Publishes npm second, also skipping an existing version.

Branch and pull-request CI compares the two manifests but does not interpret
the branch ref as a release tag.
