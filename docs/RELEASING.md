# Releasing

Widevin publishes one shared version to npm and crates.io. A release tag must
use `vX.Y.Z` and match both `package.json` and `rust/Cargo.toml`.

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

Crates.io requires the first `widevin` release to be published manually
before trusted publishing can be configured:

```sh
cargo publish --manifest-path rust/Cargo.toml
```

This is an external release action and must only be run with explicit
authorization.

After the initial release, configure the crates.io trusted publisher with:

- Repository: `dante-teo/widevin`
- Workflow: `publish.yml`
- Environment: `release`

## Subsequent Releases

After both manifests contain the intended shared version, create and push the
matching `vX.Y.Z` tag only with explicit authorization. The publish workflow:

1. Verifies TypeScript and Rust.
2. Rejects a tag that differs from either manifest.
3. Publishes crates.io first, skipping an existing version.
4. Publishes npm second, also skipping an existing version.

Branch and pull-request CI compares the two manifests but does not interpret
the branch ref as a release tag.
