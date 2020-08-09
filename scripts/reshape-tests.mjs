#!/usr/bin/env node
/**
 * Force-reshape crate `tests/` to Living 15 layout:
 *   - single Cargo integration target: tests/main.rs
 *   - sibling tests/*.rs become tests/<name>/mod.rs (subdir = not a Cargo target)
 *   - optional rename map for banned milestone identifiers
 *
 * Dry-run by default. Apply with --apply.
 *
 * Usage:
 *   node scripts/reshape-tests.mjs --crate athena-vm
 *   node scripts/reshape-tests.mjs --crate athena-graph --apply
 *   node scripts/reshape-tests.mjs --all --apply
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const args = new Set(process.argv.slice(2));
const apply = args.has("--apply");
const all = args.has("--all");

function argValue(flag) {
  const i = process.argv.indexOf(flag);
  return i >= 0 ? process.argv[i + 1] : null;
}

/** Per-crate rename of root stem → module dir name (banned milestone tokens out). */
const RENAME = {
  "athena-graph": {
    gt0_identity_contract: "identity_contract",
    gt1_capability_matrix: "capability_matrix",
    gt1_csc_invalidation: "csc_invalidation",
    gt1_deterministic_frontier: "deterministic_frontier",
    gt1_property_columns: "property_columns",
    lifecycle_p0b: "shared_chunk_lifecycle",
    graph_algorithms: "algorithms",
    graph_storage_contract: "storage_contract",
  },
};

/** Roots that must stay as separate Cargo [[test]] targets (feature-gated etc.). */
const KEEP_ROOT = {
  "athena-numeric": new Set(["ephemeral_batch"]),
};

/** Crates whose entire tests/ is forbidden (move under src #[cfg(test)] separately). */
const FORBID_TESTS_DIR = new Set(["athena-testing"]);

function listProjectCrates() {
  const projects = path.join(root, "projects");
  return fs
    .readdirSync(projects, { withFileTypes: true })
    .filter((d) => d.isDirectory() && d.name.startsWith("athena"))
    .map((d) => d.name)
    .filter((name) => fs.existsSync(path.join(projects, name, "tests")));
}

function rustIdent(stem) {
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(stem)) {
    throw new Error(`invalid rust ident from stem: ${stem}`);
  }
  return stem;
}

function planCrate(crateName) {
  const testsDir = path.join(root, "projects", crateName, "tests");
  if (!fs.existsSync(testsDir)) return null;
  if (FORBID_TESTS_DIR.has(crateName)) {
    return {
      crate: crateName,
      forbidden: true,
      actions: [
        {
          type: "note",
          message: `${crateName}: Living 15 forbids tests/ — move contract into src #[cfg(test)] manually`,
        },
      ],
    };
  }

  const keep = KEEP_ROOT[crateName] ?? new Set();
  const rename = RENAME[crateName] ?? {};
  const entries = fs.readdirSync(testsDir, { withFileTypes: true });
  const rootRs = entries.filter((e) => e.isFile() && e.name.endsWith(".rs")).map((e) => e.name);
  const actions = [];

  const moduleStems = [];
  for (const file of rootRs) {
    const stem = file.replace(/\.rs$/, "");
    if (stem === "main") continue;
    if (keep.has(stem)) {
      actions.push({ type: "keep-root", file });
      continue;
    }
    const destStem = rename[stem] ?? stem;
    rustIdent(destStem);
    const destDir = path.join(testsDir, destStem);
    const destFile = path.join(destDir, "mod.rs");
    const srcFile = path.join(testsDir, file);
    actions.push({
      type: "move-to-mod-dir",
      from: srcFile,
      to: destFile,
      stem: destStem,
      fromRel: path.relative(root, srcFile).replaceAll("\\", "/"),
      toRel: path.relative(root, destFile).replaceAll("\\", "/"),
    });
    moduleStems.push(destStem);
  }

  // Also discover existing subdirs that should be mods (already Living-shaped).
  for (const e of entries) {
    if (!e.isDirectory()) continue;
    if (e.name.startsWith(".")) continue;
    if (!moduleStems.includes(e.name) && fs.existsSync(path.join(testsDir, e.name, "mod.rs"))) {
      moduleStems.push(e.name);
    }
  }

  moduleStems.sort();
  const mainPath = path.join(testsDir, "main.rs");
  const mainBody =
    [
      `//! \`${crateName}\` 集成测试入口。`,
      ``,
      ...moduleStems.map((s) => `mod ${s};`),
      ``,
    ].join("\n") + "\n";

  actions.push({
    type: "write-main",
    path: mainPath,
    rel: path.relative(root, mainPath).replaceAll("\\", "/"),
    body: mainBody,
    modules: moduleStems,
  });

  return { crate: crateName, forbidden: false, actions, moduleStems, rootRs };
}

function applyPlan(plan) {
  for (const a of plan.actions) {
    if (a.type === "note" || a.type === "keep-root") continue;
    if (a.type === "move-to-mod-dir") {
      fs.mkdirSync(path.dirname(a.to), { recursive: true });
      if (fs.existsSync(a.to)) {
        throw new Error(`refusing to overwrite existing ${a.toRel}`);
      }
      fs.renameSync(a.from, a.to);
      continue;
    }
    if (a.type === "write-main") {
      fs.writeFileSync(a.path, a.body, "utf8");
    }
  }
}

function printPlan(plan) {
  console.log(`\n## ${plan.crate}`);
  if (plan.forbidden) {
    for (const a of plan.actions) console.log(`  NOTE  ${a.message}`);
    return;
  }
  console.log(`  current roots: ${plan.rootRs.join(", ") || "(none)"}`);
  console.log(`  modules after: ${plan.moduleStems.join(", ") || "(none)"}`);
  for (const a of plan.actions) {
    if (a.type === "keep-root") console.log(`  KEEP   tests/${a.file}`);
    else if (a.type === "move-to-mod-dir") console.log(`  MOVE   ${a.fromRel} → ${a.toRel}`);
    else if (a.type === "write-main") console.log(`  MAIN   ${a.rel}  (${a.modules.length} mods)`);
    else if (a.type === "note") console.log(`  NOTE   ${a.message}`);
  }
}

const crates = all
  ? listProjectCrates()
  : (() => {
      const c = argValue("--crate");
      if (!c) {
        console.error("usage: node scripts/reshape-tests.mjs --crate <name> [--apply] | --all [--apply]");
        process.exit(2);
      }
      return [c];
    })();

// Prefer messy crates first when --all
const priority = ["athena-vm", "athena-graph", "athena-ndarray", "athena-rewriter", "athena-types", "athena-table", "athena-testing"];
crates.sort((a, b) => {
  const ia = priority.indexOf(a);
  const ib = priority.indexOf(b);
  return (ia < 0 ? 99 : ia) - (ib < 0 ? 99 : ib) || a.localeCompare(b);
});

let plans = crates.map(planCrate).filter(Boolean);
for (const plan of plans) printPlan(plan);

if (!apply) {
  console.log("\nDry-run only. Re-run with --apply to write.");
  process.exit(0);
}

for (const plan of plans) {
  if (plan.forbidden) continue;
  applyPlan(plan);
  console.log(`applied ${plan.crate}`);
}
