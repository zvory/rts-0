#!/usr/bin/env node
import { readFileSync } from "node:fs";

function usage() {
  console.error("usage: node tests/nextest_junit_summary.mjs <junit.xml> [--limit=N]");
}

function decodeXml(value) {
  return value
    .replace(/&quot;/g, "\"")
    .replace(/&apos;/g, "'")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&");
}

function parseAttrs(tag) {
  const attrs = {};
  const attrPattern = /([A-Za-z_:][A-Za-z0-9_.:-]*)="([^"]*)"/g;
  let match;
  while ((match = attrPattern.exec(tag)) !== null) {
    attrs[match[1]] = decodeXml(match[2]);
  }
  return attrs;
}

function formatSeconds(seconds) {
  return `${seconds.toFixed(3)}s`;
}

const args = process.argv.slice(2);
const path = args.find((arg) => !arg.startsWith("--"));
const limitArg = args.find((arg) => arg.startsWith("--limit="));
const limit = Math.max(1, Number.parseInt(limitArg?.slice("--limit=".length) || "20", 10) || 20);

if (!path) {
  usage();
  process.exit(2);
}

const xml = readFileSync(path, "utf8");
const testcases = [];
const testcasePattern = /<testcase\b[^>]*>/g;
let match;
while ((match = testcasePattern.exec(xml)) !== null) {
  const attrs = parseAttrs(match[0]);
  const seconds = Number.parseFloat(attrs.time || "0");
  if (!Number.isFinite(seconds)) {
    continue;
  }
  testcases.push({
    classname: attrs.classname || "(unknown binary)",
    name: attrs.name || "(unknown test)",
    seconds,
  });
}

testcases.sort((a, b) => b.seconds - a.seconds);
const total = testcases.reduce((sum, test) => sum + test.seconds, 0);

console.log("Nextest JUnit timing summary:");
console.log(`  source: ${path}`);
console.log(`  testcases: ${testcases.length}`);
console.log(`  summed testcase time: ${formatSeconds(total)}`);
console.log(`  slowest ${Math.min(limit, testcases.length)} testcases:`);
for (const test of testcases.slice(0, limit)) {
  console.log(`    ${formatSeconds(test.seconds).padStart(10)}  ${test.classname} ${test.name}`);
}
