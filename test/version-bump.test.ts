import { spawnSync } from "node:child_process";
import {
  chmodSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";

const bumpScript = fileURLToPath(new URL("../scripts/bump-version.mjs", import.meta.url));
const fixtures: string[] = [];

const runGit = (root: string, ...arguments_: readonly string[]) =>
  spawnSync("git", arguments_, {
    cwd: root,
    encoding: "utf8"
  });

const createFixture = (
  version = "1.2.3",
  { cargoLockLineEnding = "\n" }: { readonly cargoLockLineEnding?: string } = {}
) => {
  const root = mkdtempSync(join(tmpdir(), "widevin-version-"));
  fixtures.push(root);
  mkdirSync(join(root, "rust"));
  writeFileSync(
    join(root, "package.json"),
    `${JSON.stringify({ name: "widevin", version }, null, 2)}\n`
  );
  writeFileSync(
    join(root, "rust/Cargo.toml"),
    `[package]\nname = "widevin"\nversion = "${version}"\nedition = "2024"\n`
  );
  writeFileSync(
    join(root, "rust/Cargo.lock"),
    `[[package]]\nname = "dependency"\nversion = "0.1.4"\n\n[[package]]\nname = "widevin"\nversion = "${version}"\n`.replaceAll(
      "\n",
      cargoLockLineEnding
    )
  );
  writeFileSync(
    join(root, "README.md"),
    `The TypeScript and Rust packages share version \`${version}\`.\n`
  );
  writeFileSync(
    join(root, "rust/README.md"),
    `\`\`\`toml\n[dependencies]\nwidevin = "${version}"\n\`\`\`\n`
  );
  expect(runGit(root, "init").status).toBe(0);
  expect(runGit(root, "config", "user.name", "Version Test").status).toBe(0);
  expect(runGit(root, "config", "user.email", "version-test@example.com").status).toBe(0);
  expect(runGit(root, "add", ".").status).toBe(0);
  expect(runGit(root, "commit", "-m", "fixture").status).toBe(0);
  return root;
};

const runBump = (root: string, ...arguments_: readonly string[]) =>
  spawnSync(process.execPath, [bumpScript, ...arguments_], {
    cwd: root,
    encoding: "utf8"
  });

const readVersions = (root: string) => ({
  packageJson: JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version,
  cargoToml: readFileSync(join(root, "rust/Cargo.toml"), "utf8").match(
    /^version = "([^"]+)"$/mu
  )?.[1],
  cargoLock: readFileSync(join(root, "rust/Cargo.lock"), "utf8").match(
    /\[\[package\]\]\r?\nname = "widevin"\r?\nversion = "([^"]+)"/u
  )?.[1],
  rootReadme: readFileSync(join(root, "README.md"), "utf8").match(
    /packages share version `([^`]+)`/u
  )?.[1],
  rustReadme: readFileSync(join(root, "rust/README.md"), "utf8").match(
    /^widevin = "([^"]+)"$/mu
  )?.[1]
});

afterEach(() => {
  fixtures.splice(0).forEach((fixture) => rmSync(fixture, { recursive: true }));
});

describe("shared version bump", () => {
  it("increments both package manifests, the Cargo lockfile, and current-version documentation", () => {
    const root = createFixture();

    const result = runBump(root, "patch");

    expect(result.status, result.stderr).toBe(0);
    expect(readVersions(root)).toEqual({
      packageJson: "1.2.4",
      cargoToml: "1.2.4",
      cargoLock: "1.2.4",
      rootReadme: "1.2.4",
      rustReadme: "1.2.4"
    });
    expect(result.stdout).toContain("Bumped widevin from 1.2.3 to 1.2.4");
    expect(runGit(root, "status", "--porcelain").stdout).toBe("");
    expect(runGit(root, "log", "-1", "--pretty=%s").stdout.trim()).toBe("1.2.4");
    expect(runGit(root, "cat-file", "-t", "v1.2.4").stdout.trim()).toBe("tag");
  });

  it("preserves CRLF line endings while bumping Cargo.lock", () => {
    const root = createFixture("1.2.3", { cargoLockLineEnding: "\r\n" });

    const result = runBump(root, "patch");

    expect(result.status, result.stderr).toBe(0);
    expect(readVersions(root).cargoLock).toBe("1.2.4");
    const cargoLock = readFileSync(join(root, "rust/Cargo.lock"), "utf8");
    expect(cargoLock).toContain("\r\n");
    expect(cargoLock.replaceAll("\r\n", "")).not.toContain("\n");
  });

  it("supports prerelease increments and explicit semantic versions", () => {
    const prereleaseRoot = createFixture();
    const inlinePrereleaseRoot = createFixture();
    const explicitRoot = createFixture();

    expect(runBump(prereleaseRoot, "prerelease", "--preid", "beta").status).toBe(0);
    expect(runBump(inlinePrereleaseRoot, "prerelease", "--preid=rc").status).toBe(0);
    expect(runBump(explicitRoot, "2.0.0").status).toBe(0);

    expect(new Set(Object.values(readVersions(prereleaseRoot)))).toEqual(
      new Set(["1.2.4-beta.0"])
    );
    expect(new Set(Object.values(readVersions(inlinePrereleaseRoot)))).toEqual(
      new Set(["1.2.4-rc.0"])
    );
    expect(new Set(Object.values(readVersions(explicitRoot)))).toEqual(new Set(["2.0.0"]));
  });

  it("rejects invalid bumps without changing any file", () => {
    const root = createFixture();
    const before = readVersions(root);

    const result = runBump(root, "banana");

    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain("Expected a semantic version or one of");
    expect(result.stderr).not.toContain("bump-version.mjs:");
    expect(readVersions(root)).toEqual(before);
  });

  it("rejects existing version drift before changing any file", () => {
    const root = createFixture();
    const cargoTomlPath = join(root, "rust/Cargo.toml");
    writeFileSync(
      cargoTomlPath,
      readFileSync(cargoTomlPath, "utf8").replace('version = "1.2.3"', 'version = "1.2.2"')
    );
    expect(runGit(root, "add", cargoTomlPath).status).toBe(0);
    expect(runGit(root, "commit", "-m", "introduce drift").status).toBe(0);
    const before = {
      packageJson: readFileSync(join(root, "package.json"), "utf8"),
      cargoToml: readFileSync(cargoTomlPath, "utf8")
    };

    const result = runBump(root, "patch");

    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain("does not match");
    expect(readFileSync(join(root, "package.json"), "utf8")).toBe(before.packageJson);
    expect(readFileSync(cargoTomlPath, "utf8")).toBe(before.cargoToml);
  });

  it("rejects a dirty worktree without changing version files", () => {
    const root = createFixture();
    writeFileSync(join(root, "notes.txt"), "not ready\n");
    const before = readVersions(root);

    const result = runBump(root, "patch");

    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain("working tree must be clean");
    expect(readVersions(root)).toEqual(before);
    expect(readFileSync(join(root, "notes.txt"), "utf8")).toBe("not ready\n");
  });

  it("rejects an existing release tag before changing version files", () => {
    const root = createFixture();
    expect(runGit(root, "tag", "-a", "v1.2.4", "-m", "existing").status).toBe(0);
    const before = readVersions(root);

    const result = runBump(root, "patch");

    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain("Tag v1.2.4 already exists");
    expect(readVersions(root)).toEqual(before);
    expect(runGit(root, "log", "-1", "--pretty=%s").stdout.trim()).toBe("fixture");
  });

  it("rolls back file and index changes when the release commit fails", () => {
    const root = createFixture();
    expect(runGit(root, "config", "user.name", "").status).toBe(0);
    const before = readVersions(root);

    const result = runBump(root, "patch");

    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain("git commit");
    expect(readVersions(root)).toEqual(before);
    expect(runGit(root, "status", "--porcelain").stdout).toBe("");
    expect(runGit(root, "log", "-1", "--pretty=%s").stdout.trim()).toBe("fixture");
    expect(runGit(root, "tag", "--list", "v1.2.4").stdout).toBe("");
  });

  it.skipIf(process.platform === "win32")(
    "rolls back partial file writes when a version file cannot be written",
    () => {
      const root = createFixture();
      const readmePath = join(root, "README.md");
      chmodSync(readmePath, 0o444);
      const before = readVersions(root);

      const result = runBump(root, "patch");

      expect(result.status).not.toBe(0);
      expect(readVersions(root)).toEqual(before);
      expect(runGit(root, "status", "--porcelain").stdout).toBe("");
      expect(runGit(root, "log", "-1", "--pretty=%s").stdout.trim()).toBe("fixture");
      expect(runGit(root, "tag", "--list", "v1.2.4").stdout).toBe("");
    }
  );

  it("rolls back the release commit when annotated tag creation fails", () => {
    const root = createFixture();
    expect(runGit(root, "config", "tag.gpgSign", "true").status).toBe(0);
    expect(
      runGit(root, "config", "user.signingkey", "widevin-version-test-missing-key").status
    ).toBe(0);
    const before = readVersions(root);

    const result = runBump(root, "patch");

    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain("git tag");
    expect(readVersions(root)).toEqual(before);
    expect(runGit(root, "status", "--porcelain").stdout).toBe("");
    expect(runGit(root, "log", "-1", "--pretty=%s").stdout.trim()).toBe("fixture");
    expect(runGit(root, "tag", "--list", "v1.2.4").stdout).toBe("");
  });

  it("does not push the release commit or tag", () => {
    const root = createFixture();
    const remote = mkdtempSync(join(tmpdir(), "widevin-version-remote-"));
    fixtures.push(remote);
    expect(runGit(remote, "init", "--bare").status).toBe(0);
    expect(runGit(root, "remote", "add", "origin", remote).status).toBe(0);
    expect(runGit(root, "push", "--set-upstream", "origin", "HEAD").status).toBe(0);
    const branch = runGit(root, "branch", "--show-current").stdout.trim();
    const remoteHeadBefore = runGit(
      root,
      "ls-remote",
      "--heads",
      "origin",
      `refs/heads/${branch}`
    ).stdout;

    expect(runBump(root, "patch").status).toBe(0);

    expect(
      runGit(root, "ls-remote", "--heads", "origin", `refs/heads/${branch}`).stdout
    ).toBe(remoteHeadBefore);
    expect(runGit(root, "ls-remote", "--tags", "origin", "v1.2.4").stdout).toBe("");
  });
});
