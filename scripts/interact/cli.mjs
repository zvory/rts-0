#!/usr/bin/env node

const [major = 0, minor = 0] = process.versions.node.split(".").map(Number);
if (major < 22 || (major === 22 && minor < 18)) {
  process.stderr.write("Interact requires Node 22.18 or newer.\n");
  process.exitCode = 1;
} else {
  const { main } = await import("./cli.ts");
  await main();
}
