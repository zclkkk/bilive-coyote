import { mkdirSync, rmSync } from "fs";
import { join } from "path";

const APP_NAME = "bilive-coyote";
const DIST_DIR = "dist";
const ENTRYPOINT = "src/main.ts";
const TARGETS = [
  "bun-linux-x64",
  "bun-linux-arm64",
  "bun-windows-x64",
  "bun-windows-arm64",
  "bun-darwin-x64",
  "bun-darwin-arm64",
] as const;
type Target = (typeof TARGETS)[number];

const targets = process.argv.slice(2);
const invalidTarget = targets.find((target) => !TARGETS.includes(target as Target));

if (invalidTarget) {
  console.error(`Unknown target: ${invalidTarget}`);
  process.exit(1);
}

const builds: Array<Target | null> = targets.length > 0 ? (targets as Target[]) : [null];

const check = Bun.spawnSync(["bun", "run", "tsc", "--noEmit"], { stdio: ["inherit", "inherit", "inherit"] });
if (check.exitCode !== 0) process.exit(check.exitCode ?? 1);

rmSync(DIST_DIR, { recursive: true, force: true });
mkdirSync(DIST_DIR, { recursive: true });

for (const target of builds) {
  const outfile = join(DIST_DIR, getOutputName(target));
  const result = await Bun.build({
    entrypoints: [ENTRYPOINT],
    compile: target ? { target, outfile } : { outfile },
    minify: true,
  });

  if (!result.success) {
    for (const log of result.logs) console.error(log);
    process.exit(1);
  }

  console.log(`Built ${outfile}`);
}

function getOutputName(target: Target | null): string {
  if (!target) return APP_NAME;

  const suffix = target.replace(/^bun-/, "");
  const extension = target.includes("windows") ? ".exe" : "";
  return `${APP_NAME}-${suffix}${extension}`;
}
