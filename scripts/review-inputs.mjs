import { spawnSync } from "node:child_process";

export const MAX_RAW_REVIEW_BYTES = 256 * 1024;

const GENERATED_PATHS = [
  /^client\/assets\/snapshot-streams\//,
  /^server\/assets\/lab-scenarios\/.*\.json$/,
  /(?:^|\/)generated\//,
];

function runGit(repoRoot, args, { allowFailure = false } = {}) {
  const result = spawnSync("git", args, {
    cwd: repoRoot,
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.error) throw result.error;
  if (result.status !== 0 && !allowFailure) {
    throw new Error(result.stderr?.trim() || result.stdout?.trim() || `git exited ${result.status}`);
  }
  return result.status === 0 ? result.stdout.trim() : "";
}

function blobMetadata(repoRoot, ref, pathname) {
  const object = runGit(repoRoot, ["rev-parse", "--verify", `${ref}:${pathname}`], { allowFailure: true });
  if (!object) return { bytes: 0, sha256: "absent" };
  const bytes = Number(runGit(repoRoot, ["cat-file", "-s", object]));
  return { bytes, sha256: object };
}

export function classifyReviewPath(pathname, { binary = false, newBytes = 0, oldBytes = 0 } = {}) {
  if (binary) return "binary";
  if (GENERATED_PATHS.some((pattern) => pattern.test(pathname))) return "generated";
  if (Math.max(newBytes, oldBytes) > MAX_RAW_REVIEW_BYTES) return "large-text";
  return "reviewable";
}

export function collectReviewInputs({ repoRoot, baseRef, headRef = "HEAD" }) {
  const base = runGit(repoRoot, ["merge-base", baseRef, headRef]);
  const paths = runGit(repoRoot, ["diff", "--name-only", "--no-renames", `${base}..${headRef}`])
    .split("\n")
    .filter(Boolean);
  return paths.map((pathname) => {
    const oldBlob = blobMetadata(repoRoot, base, pathname);
    const newBlob = blobMetadata(repoRoot, headRef, pathname);
    const numstat = runGit(repoRoot, ["diff", "--numstat", "--no-renames", `${base}..${headRef}`, "--", pathname]);
    const [added = "0", deleted = "0"] = numstat.split("\t");
    const binary = added === "-" || deleted === "-";
    return {
      path: pathname,
      classification: classifyReviewPath(pathname, {
        binary,
        newBytes: newBlob.bytes,
        oldBytes: oldBlob.bytes,
      }),
      oldBytes: oldBlob.bytes,
      newBytes: newBlob.bytes,
      oldBlob: oldBlob.sha256,
      newBlob: newBlob.sha256,
      added: binary ? null : Number(added),
      deleted: binary ? null : Number(deleted),
    };
  });
}

export function renderReviewInputManifest(inputs) {
  if (inputs.length === 0) return "<no changed paths>";
  return inputs.map((input) => {
    const stat = input.added === null
      ? "binary diff"
      : `+${input.added}/-${input.deleted}`;
    return [
      input.path,
      `class=${input.classification}`,
      `bytes=${input.oldBytes}->${input.newBytes}`,
      `blobs=${input.oldBlob}->${input.newBlob}`,
      stat,
    ].join(" | ");
  }).join("\n");
}

export function excludedRawPaths(inputs) {
  return inputs
    .filter((input) => input.classification !== "reviewable")
    .map((input) => input.path);
}
