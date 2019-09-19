#!/usr/bin/env node
/**
 * Living 27 source gate: ban Mathematica-shaped semantic dispatch in core crates.
 * Allows diagnostics / debug_label / residual Extension display / test fixtures.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const scanRoots = [
  "projects/athena-engine/src",
  "projects/athena-ir/src",
  "projects/athena/src",
  "projects/athena-testing/src",
];

/** Patterns that must not appear in production src (dispatch / construction). */
const banned = [
  { re: /\btry_calculus_request\b/, why: "string calculus hub" },
  { re: /\blookup_function\s*\(/, why: "string function registry" },
  { re: /push_application\(\s*[^,]+,\s*"(Plus|Times|Simplify|Integrate|Limit|Define|LinearSolve|Cond|Recover|Sin|Cos)"/, why: "named-head core construction" },
  { re: /push_application_named\(\s*[^,]+,\s*"(Plus|Times|Simplify|Integrate|Limit|Define|LinearSolve|Cond|Recover|Sin|Cos)"/, why: "named-head core construction" },
  { re: /operators\.intern\(\s*"(Plus|Times|Simplify|Integrate|Limit|Define|LinearSolve|Cond|Recover|Sin|Cos)"\s*\)/, why: "Extension intern of core surface name" },
  { re: /match\s+name\.as_str\(\)\s*\{[\s\S]{0,200}"(LinearSolve|Import|Export|Timing|Define|CountedLoop|Cond|Recover)"/, why: "Extension surface-name match dispatch" },
];

const allowPath = (rel) => {
  // Unit tests under src may keep Extension residual fixtures.
  if (rel.includes("/tests/") || rel.endsWith("/tests.rs")) return true;
  if (rel.includes("\\tests\\") || rel.endsWith("\\tests.rs")) return true;
  return false;
};

let failures = [];

for (const relRoot of scanRoots) {
  const absRoot = path.join(root, relRoot);
  if (!fs.existsSync(absRoot)) continue;
  walk(absRoot, (file) => {
    if (!file.endsWith(".rs")) return;
    const rel = path.relative(root, file).replace(/\\/g, "/");
    if (allowPath(rel)) return;
    const text = fs.readFileSync(file, "utf8");
    for (const { re, why } of banned) {
      const m = text.match(re);
      if (m) {
        failures.push(`${rel}: ${why} (matched ${JSON.stringify(m[0].slice(0, 80))})`);
      }
    }
  });
}

if (failures.length) {
  console.error("Neutral surface gate failed:\n" + failures.map((f) => "  - " + f).join("\n"));
  process.exit(1);
}
console.log("Neutral surface gate OK");

function walk(dir, visit) {
  for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, ent.name);
    if (ent.isDirectory()) walk(p, visit);
    else visit(p);
  }
}
