import { readFile } from "node:fs/promises";

const [packageJson, cargoToml] = await Promise.all([
  readFile(new URL("../package.json", import.meta.url), "utf8").then(JSON.parse),
  readFile(new URL("../rust/Cargo.toml", import.meta.url), "utf8")
]);
const cargoVersion = cargoToml.match(/^version = "([^"]+)"$/mu)?.[1];
const packageVersion = packageJson.version;
const tagVersion =
  process.env.GITHUB_REF_TYPE === "tag" ? process.env.GITHUB_REF_NAME?.replace(/^v/u, "") : undefined;

if (!cargoVersion) throw new Error("Could not read package version from rust/Cargo.toml");
if (packageVersion !== cargoVersion) {
  throw new Error(`package.json ${packageVersion} does not match rust/Cargo.toml ${cargoVersion}`);
}
if (tagVersion && tagVersion !== packageVersion) {
  throw new Error(`Tag v${tagVersion} does not match package version ${packageVersion}`);
}

console.log(`Version ${packageVersion} matches both package manifests${tagVersion ? " and tag" : ""}.`);
