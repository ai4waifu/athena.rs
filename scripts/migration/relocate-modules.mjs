#!/usr/bin/env node
/**
 * Deterministic Athena module relocation helper.
 *
 * Mechanical moves only — no semantic edits. Run from repo root:
 *   node scripts/migration/relocate-modules.mjs [--dry-run] [--apply]
 *
 * Default is dry-run. `--apply` performs git mv + text rewrites listed below.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "../..");
const apply = process.argv.includes("--apply");
const dryRun = !apply;

/** @type {{ from: string, to: string }[]} */
const moves = [
  {
    from: "projects/athena-ir/src/arena",
    to: "projects/athena-ir/src/store",
  },
  {
    from: "projects/athena-ir/tests/arena",
    to: "projects/athena-ir/tests/store",
  },
];

/** Path-relative text rewrites after moves (exact string replace). */
const rewrites = [
  {
    file: "projects/athena-ir/src/lib.rs",
    replacements: [
      ["pub mod arena;", "pub mod store;"],
      ["pub use arena::TermStore;", "pub use store::TermStore;"],
      ["term arena、节点、builder、验证。", "TermStore、节点、builder、验证。"],
    ],
  },
  {
    file: "projects/athena-ir/src/build/builder.rs",
    replacements: [["arena::TermStore", "store::TermStore"]],
  },
  {
    file: "projects/athena-ir/src/canonical/fingerprint.rs",
    replacements: [["arena::TermStore", "store::TermStore"]],
  },
  {
    file: "projects/athena-ir/tests/main.rs",
    replacements: [["mod arena;", "mod store;"]],
  },
  {
    file: "projects/athena-ir/src/store/mod.rs",
    replacements: [
      ["//! Term arena — Core IR 唯一存储。", "//! `TermStore` — Core IR 唯一符号项存储。"],
      ["/// 基于 arena 的 Core CAS IR。", "/// Core CAS IR 符号项存储。"],
      ["/// 空 arena。", "/// 空存储。"],
    ],
  },
  {
    file: "projects/athena-ir/Cargo.toml",
    replacements: [
      [
        'description = "athena Core CAS IR — arena, nodes, builder, verifier"',
        'description = "athena Core CAS IR — TermStore, nodes, builder, verifier"',
      ],
    ],
  },
];

function exists(rel) {
  return fs.existsSync(path.join(root, rel));
}

function gitMv(from, to) {
  const absToParent = path.dirname(path.join(root, to));
  fs.mkdirSync(absToParent, { recursive: true });
  const r = spawnSync("git", ["mv", from, to], { cwd: root, encoding: "utf8" });
  if (r.status !== 0) {
    throw new Error(`git mv ${from} → ${to} failed: ${r.stderr || r.stdout}`);
  }
}

function applyRewrites() {
  for (const entry of rewrites) {
    const abs = path.join(root, entry.file);
    if (!fs.existsSync(abs)) {
      console.warn(`skip missing rewrite target: ${entry.file}`);
      continue;
    }
    let text = fs.readFileSync(abs, "utf8");
    let changed = false;
    for (const [from, to] of entry.replacements) {
      if (!text.includes(from)) {
        console.warn(`  miss ${entry.file}: ${JSON.stringify(from)}`);
        continue;
      }
      text = text.split(from).join(to);
      changed = true;
      console.log(`  rewrite ${entry.file}: ${JSON.stringify(from)} → ${JSON.stringify(to)}`);
    }
    if (changed && apply) {
      fs.writeFileSync(abs, text, "utf8");
    }
  }
}

console.log(dryRun ? "dry-run (pass --apply to mutate)" : "applying relocation");
for (const { from, to } of moves) {
  const fromOk = exists(from);
  const toOk = exists(to);
  console.log(`move ${from} → ${to} (from=${fromOk} to=${toOk})`);
  if (!fromOk) {
    console.warn(`  skip: source missing`);
    continue;
  }
  if (toOk) {
    console.warn(`  skip: destination already exists`);
    continue;
  }
  if (apply) {
    gitMv(from, to);
  }
}
applyRewrites();
console.log("done");
