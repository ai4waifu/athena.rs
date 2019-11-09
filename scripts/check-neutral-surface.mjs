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
  "projects/athena-types/src",
  "projects/athena-gc/src",
  "projects/athena-numeric/src",
  "projects/athena-ir/src",
  "projects/athena-rewriter/src",
  "projects/athena-engine/src",
  "projects/athena/src",
  "projects/athena-testing/src",
];

/** Patterns that must not appear in production src (dispatch / construction). */
const banned = [
  { re: /\bCalculusCtx\b/, why: "CalculusCtx mini-evaluator (Living 28)" },
  { re: /\bextension_named\s*\(/, why: "extension display-name semantic check" },
  { re: /fn\s+apply\s*\(\s*&self,\s*head:\s*&str/, why: "string apply construction API" },
  { re: /\blookup_function\s*\(/, why: "string function registry" },
  { re: /\bpush_application_named\s*\(/, why: "string named application construction" },
  { re: /fn\s+push_application\s*\([^)]*&str/, why: "push_application(&str) construction API" },
  { re: /\btry_calculus_request\s*\(/, why: "string calculus goal hub" },
  { re: /\bhead_name_session\s*\(/, why: "string head semantic hub" },
  { re: /\bterm_head_name\s*\(/, why: "string head semantic hub" },
  { re: /\bhead_name\s*[:=(]/, why: "string head semantic hub" },
  { re: /\.as_deref\(\)\s*==\s*Some\(\s*"Pi"\s*\)/, why: "symbol-name Pi constant check" },
  { re: /\.as_deref\(\)\s*==\s*Some\(\s*"E"\s*\)/, why: "symbol-name E constant check" },
  { re: /Some\(\s*"True"\s*\)\s*=>/, why: "symbol-name True boolean arm" },
  { re: /Some\(\s*"False"\s*\)\s*=>/, why: "symbol-name False boolean arm" },
  {
    re: /push_application(?:_named)?\(\s*[^,]+,\s*"(Plus|Times|Simplify|Integrate|Limit|Define|LinearSolve|Cond|Recover|Sin|Cos|D|SetDelayed|RuleDelayed|OwnValue|DownValue|Blank)"/,
    why: "named-head core construction",
  },
  {
    re: /operators\.intern\(\s*"(Plus|Times|Simplify|Integrate|Limit|Define|LinearSolve|Cond|Recover|Sin|Cos|D|SetDelayed|RuleDelayed|OwnValue|DownValue)"\s*\)/,
    why: "Extension intern of core surface name",
  },
  {
    re: /match\s+name\.as_str\(\)\s*\{[\s\S]{0,240}"(LinearSolve|Import|Export|Timing|Define|CountedLoop|Cond|Recover|Plus|Times|Integrate|Limit|D)"/,
    why: "Extension surface-name match dispatch",
  },
  {
    re: /match\s+h\.as_str\(\)\s*\{/,
    why: "string head match dispatch",
  },
  // Mathematica surface names as Rust string literals (not neutral debug_label Add/Multiply).
  {
    re: /"(Plus|Times|SetDelayed|RuleDelayed|OwnValue|DownValue|HoldAll|BlankSequence|BlankNullSequence|WriteDownValue)"/,
    why: "Mathematica surface string literal in core",
  },
];

/** Paths allowed to mention surface strings (display / docs / fixtures). */
const allowPath = (rel) => {
  if (rel.includes("/tests/") || rel.endsWith("/tests.rs")) return true;
  if (rel.includes("\\tests\\") || rel.endsWith("\\tests.rs")) return true;
  // Neutral debug_label may still spell Integrate / Limit (operator names, not Plus).
  if (rel.endsWith("/operator/semantic.rs")) return false;
  // Boundary negative tests live under src sometimes.
  if (rel.includes("/boundary/")) return true;
  return false;
};

/** Per-file allow for specific ban reasons (display helpers only). */
const allowMatch = (rel, why) => {
  if (why === "Mathematica surface string literal in core") {
    // No core file should carry Plus/Times literals today.
    return false;
  }
  if (why === "string head semantic hub") {
    // `application_display_name` / comments mentioning head_name history are ok if we only ban `head_name` token as API.
    // Keep strict: only allow diagnostics comments that don't define the API.
    if (rel.includes("/diagnostics/")) return true;
  }
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
    // Strip line comments so doc examples like `head_name: String` do not trip the gate.
    const code = text
      .split("\n")
      .map((line) => {
        const t = line.trimStart();
        if (t.startsWith("//") || t.startsWith("*") || t.startsWith("//!") || t.startsWith("///")) {
          return "";
        }
        return line;
      })
      .join("\n");
    for (const { re, why } of banned) {
      const m = code.match(re);
      if (m) {
        if (allowMatch(rel, why)) continue;
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
