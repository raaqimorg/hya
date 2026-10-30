// Checks that a version is ready to release, then prints its CHANGELOG.md section: the notes of
// the GitHub Release. Every version in the repository must equal it, so a tag can never publish
// crates and a package that disagree.
// Run: bun scripts/release-notes.ts 0.2.0   (the release workflow passes the tag without its v)
import { readFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dir, "..");
const read = (path: string): string => readFileSync(join(root, path), "utf8");

const version = process.argv[2] ?? "";
if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version)) {
  console.error(`usage: bun scripts/release-notes.ts <version>, e.g. 0.2.0 (got "${version}")`);
  process.exit(1);
}

type Cargo = {
  workspace: {
    package: { version: string };
    dependencies: Record<string, { version?: string } | string | undefined>;
  };
};
const cargo = Bun.TOML.parse(read("Cargo.toml")) as Cargo;
const dependency = (name: string): string | undefined => {
  const entry = cargo.workspace.dependencies[name];
  return typeof entry === "object" ? entry.version : undefined;
};
const npm = JSON.parse(read("packages/js/package.json")) as { version: string };

const found: [string, string | undefined][] = [
  ["Cargo.toml [workspace.package] version", cargo.workspace.package.version],
  ["Cargo.toml hya-engine dependency", dependency("hya-engine")],
  ["Cargo.toml hya-countries dependency", dependency("hya-countries")],
  ["packages/js/package.json version", npm.version],
];
const wrong = found.filter(([, value]) => value !== version);
for (const [where, value] of wrong) console.error(`${where} is ${value}, not ${version}`);

// The section runs from its `## [x.y.z] - YYYY-MM-DD` heading to the next `## ` heading.
const changelog = read("CHANGELOG.md");
const escaped = version.replaceAll(".", "\\.");
const section = new RegExp(`^## \\[${escaped}\\] - \\d{4}-\\d{2}-\\d{2}\\n([\\s\\S]*?)(?=^## |^\\[)`, "m").exec(
  changelog,
);
if (!section) console.error(`CHANGELOG.md has no "## [${version}] - YYYY-MM-DD" section`);
const notes = section?.[1]?.trim() ?? "";
if (section && notes === "") console.error(`CHANGELOG.md section ${version} is empty`);

if (wrong.length > 0 || notes === "") process.exit(1);
console.log(notes);
