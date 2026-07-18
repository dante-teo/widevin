import { spawnSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { relative, resolve } from "node:path";
import { inc, valid } from "semver";

const RELEASE_TYPES = [
  "major",
  "minor",
  "patch",
  "premajor",
  "preminor",
  "prepatch",
  "prerelease"
];
const PRE_RELEASE_TYPES = new Set(["premajor", "preminor", "prepatch", "prerelease"]);
const USAGE =
  "Usage: pnpm bump <major|minor|patch|premajor|preminor|prepatch|prerelease|version> [--preid <name>]";

const parsePreid = (options) => {
  if (options.length === 0) return undefined;
  if (options.length === 2 && options[0] === "--preid" && options[1]) {
    return options[1];
  }
  if (options.length === 1 && options[0]?.startsWith("--preid=")) {
    const preid = options[0].slice("--preid=".length);
    if (preid) return preid;
  }
  throw new Error(USAGE);
};

const parseArguments = ([requestedVersion, ...options]) => {
  if (!requestedVersion) throw new Error(USAGE);
  const preid = parsePreid(options);
  if (preid && !PRE_RELEASE_TYPES.has(requestedVersion)) {
    throw new Error("--preid can only be used with a prerelease bump");
  }

  return { requestedVersion, preid };
};

const nextVersionFor = (currentVersion, requestedVersion, preid) => {
  const explicitVersion = valid(requestedVersion);
  if (explicitVersion) return explicitVersion;
  if (!RELEASE_TYPES.includes(requestedVersion)) {
    throw new Error(
      `Expected a semantic version or one of: ${RELEASE_TYPES.join(", ")}`
    );
  }

  const nextVersion = inc(currentVersion, requestedVersion, preid);
  if (!nextVersion) throw new Error(`Could not increment invalid version ${currentVersion}`);
  return nextVersion;
};

const matchVersion = (contents, pattern, label) => {
  const version = contents.match(pattern)?.[1];
  if (!version) throw new Error(`Could not read ${label} version`);
  return version;
};

const replaceVersion = (contents, pattern, nextVersion, label) => {
  let replacements = 0;
  const updated = contents.replace(pattern, (_match, before, after) => {
    replacements += 1;
    return `${before}${nextVersion}${after}`;
  });
  if (replacements !== 1) {
    throw new Error(`Expected exactly one current-version reference in ${label}`);
  }
  return updated;
};

const writeUpdates = async (updates) => {
  const results = await Promise.allSettled(
    Object.entries(updates).map(([path, contents]) => writeFile(path, contents, "utf8"))
  );
  const failure = results.find((result) => result.status === "rejected");
  if (failure) throw failure.reason;
};

const runGit = (root, arguments_, { allowFailure = false } = {}) => {
  const result = spawnSync("git", arguments_, {
    cwd: root,
    encoding: "utf8"
  });
  if (result.error) throw result.error;
  if (!allowFailure && result.status !== 0) {
    const detail = result.stderr.trim() || result.stdout.trim();
    throw new Error(`git ${arguments_.join(" ")} failed${detail ? `: ${detail}` : ""}`);
  }
  return result;
};

const assertCleanReleaseState = (root, tag) => {
  const repository = runGit(root, ["rev-parse", "--is-inside-work-tree"]);
  if (repository.stdout.trim() !== "true") {
    throw new Error("pnpm bump must run inside a Git working tree");
  }

  const status = runGit(root, ["status", "--porcelain", "--untracked-files=all"]);
  if (status.stdout.trim()) {
    throw new Error("The Git working tree must be clean before bumping the version");
  }

  const existingTag = runGit(
    root,
    ["rev-parse", "--verify", "--quiet", `refs/tags/${tag}`],
    { allowFailure: true }
  );
  if (existingTag.status === 0) throw new Error(`Tag ${tag} already exists`);
  if (existingTag.status !== 1) {
    throw new Error(`Could not check whether tag ${tag} already exists`);
  }
};

const rollbackRelease = (root, versionPaths, originalHead) => {
  const currentHead = runGit(root, ["rev-parse", "HEAD"]).stdout.trim();
  if (currentHead !== originalHead) {
    runGit(root, [
      "update-ref",
      "-m",
      "rollback failed pnpm bump",
      "HEAD",
      originalHead,
      currentHead
    ]);
  }
  runGit(root, [
    "restore",
    "--source=HEAD",
    "--staged",
    "--worktree",
    "--",
    ...versionPaths
  ]);
};

const main = async () => {
  const root = process.cwd();
  const paths = {
    packageJson: resolve(root, "package.json"),
    cargoToml: resolve(root, "rust/Cargo.toml"),
    cargoLock: resolve(root, "rust/Cargo.lock"),
    rootReadme: resolve(root, "README.md"),
    rustReadme: resolve(root, "rust/README.md")
  };

  const [packageJsonSource, cargoToml, cargoLock, rootReadme, rustReadme] =
    await Promise.all(Object.values(paths).map((path) => readFile(path, "utf8")));
  const packageJson = JSON.parse(packageJsonSource);
  const currentVersion = packageJson.version;
  const currentVersions = {
    "rust/Cargo.toml": matchVersion(
      cargoToml,
      /^version = "([^"]+)"$/mu,
      "rust/Cargo.toml"
    ),
    "rust/Cargo.lock": matchVersion(
      cargoLock,
      /\[\[package\]\]\r?\nname = "widevin"\r?\nversion = "([^"]+)"/u,
      "rust/Cargo.lock"
    ),
    "README.md": matchVersion(
      rootReadme,
      /packages share version `([^`]+)`/u,
      "README.md"
    ),
    "rust/README.md": matchVersion(
      rustReadme,
      /^widevin = "([^"]+)"$/mu,
      "rust/README.md"
    )
  };

  if (typeof currentVersion !== "string" || !valid(currentVersion)) {
    throw new Error("Could not read a valid package.json version");
  }
  Object.entries(currentVersions).forEach(([label, version]) => {
    if (version !== currentVersion) {
      throw new Error(`${label} ${version} does not match package.json ${currentVersion}`);
    }
  });

  const { requestedVersion, preid } = parseArguments(process.argv.slice(2));
  const nextVersion = nextVersionFor(currentVersion, requestedVersion, preid);
  if (nextVersion === currentVersion) {
    throw new Error(`Version is already ${currentVersion}`);
  }
  const tag = `v${nextVersion}`;
  assertCleanReleaseState(root, tag);
  const originalHead = runGit(root, ["rev-parse", "HEAD"]).stdout.trim();

  const updates = {
    [paths.packageJson]: `${JSON.stringify({ ...packageJson, version: nextVersion }, null, 2)}\n`,
    [paths.cargoToml]: replaceVersion(
      cargoToml,
      /(^version = ")[^"]+(")$/gmu,
      nextVersion,
      "rust/Cargo.toml"
    ),
    [paths.cargoLock]: replaceVersion(
      cargoLock,
      /(\[\[package\]\]\r?\nname = "widevin"\r?\nversion = ")[^"]+(")/gu,
      nextVersion,
      "rust/Cargo.lock"
    ),
    [paths.rootReadme]: replaceVersion(
      rootReadme,
      /(packages share version `)[^`]+(`)/gu,
      nextVersion,
      "README.md"
    ),
    [paths.rustReadme]: replaceVersion(
      rustReadme,
      /(^widevin = ")[^"]+(")$/gmu,
      nextVersion,
      "rust/README.md"
    )
  };

  const versionPaths = Object.keys(updates).map((path) => relative(root, path));
  try {
    await writeUpdates(updates);
    runGit(root, ["add", "--", ...versionPaths]);
    runGit(root, ["commit", "-m", nextVersion]);
    runGit(root, ["tag", "--annotate", tag, "--message", tag]);
  } catch (error) {
    try {
      rollbackRelease(root, versionPaths, originalHead);
    } catch (rollbackError) {
      const detail =
        rollbackError instanceof Error ? rollbackError.message : String(rollbackError);
      throw new Error(
        `${error instanceof Error ? error.message : String(error)}; rollback failed: ${detail}`
      );
    }
    throw error;
  }
  console.log(
    `Bumped widevin from ${currentVersion} to ${nextVersion}, committed, and tagged ${tag}.`
  );
};

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
