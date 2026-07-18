import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const runVersionCheck = (environment: NodeJS.ProcessEnv) =>
  spawnSync(process.execPath, ["scripts/check-versions.mjs"], {
    cwd: new URL("..", import.meta.url),
    encoding: "utf8",
    env: { ...process.env, ...environment }
  });

describe("release metadata", () => {
  it("ignores branch refs while still rejecting mismatched release tags", () => {
    const branch = runVersionCheck({
      GITHUB_REF_NAME: "main",
      GITHUB_REF_TYPE: "branch"
    });
    expect(branch.status, branch.stderr).toBe(0);

    const tag = runVersionCheck({
      GITHUB_REF_NAME: "v0.1.5",
      GITHUB_REF_TYPE: "tag"
    });
    expect(tag.status).not.toBe(0);
    expect(tag.stderr).toContain("does not match package version 0.1.4");
  });

  it("includes the protocol provenance notice in the Rust crate", () => {
    const cargoToml = readFileSync(new URL("../rust/Cargo.toml", import.meta.url), "utf8");
    const notice = readFileSync(new URL("../rust/NOTICE", import.meta.url), "utf8");

    expect(cargoToml).toMatch(/^include = \[.*"NOTICE".*\]$/mu);
    expect(notice).toContain("can1357/oh-my-pi");
    expect(notice).toContain("rust/src/proto.rs");
  });
});
