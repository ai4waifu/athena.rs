#!/usr/bin/env node
/**
 * Crate dependency-direction gate for Athena layering.
 *
 * Frozen edges (must hold):
 *   athena-vm must not depend on athena-engine / frontend / domain crates
 *   athena-gc must not depend on athena-vm / athena-engine / athena-ir
 *   athena-types must not depend on upper crates
 *   athena-ir must not depend on athena-engine / athena-vm
 *   athena-rewriter must not depend on athena-engine / athena-vm
 *
 * Also bans DomainGoal / Claim / PolynomialStore-shaped names from athena-vm
 * public `src/lib.rs` re-exports (boundary regression signal).
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/** Extract direct dependency package names from a Cargo.toml text. */
function depNames(tomlText) {
  const names = new Set();
  const sections = ["dependencies", "dev-dependencies", "build-dependencies"];
  let active = null;
  for (const raw of tomlText.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const sec = line.match(/^\[([^\]]+)\]$/);
    if (sec) {
      const name = sec[1];
      active = sections.includes(name) || sections.some((s) => name.endsWith(`.${s}`)) ? name : null;
      continue;
    }
    if (!active) continue;
    const m = line.match(/^([A-Za-z0-9_-]+)\s*=/);
    if (m) names.add(m[1]);
  }
  return names;
}

function readToml(rel) {
  return fs.readFileSync(path.join(root, rel), "utf8");
}

const rules = [
  {
    crate: "projects/athena-vm/Cargo.toml",
    forbid: [
      "athena-engine",
      "athena",
      "athena-testing",
      "athena-benchmark",
      "athena-jit",
      "athena-graph",
      "athena-ndarray",
      "athena-table",
      "athena-rewriter",
      "athena-numeric",
      "athena-ir",
      "sxo",
      "sxo-napi",
      "sxo-types",
    ],
    why: "athena-vm is a restricted ExecutionIR runtime (engine above it)",
  },
  {
    crate: "projects/athena-gc/Cargo.toml",
    forbid: ["athena-vm", "athena-engine", "athena-ir", "athena-rewriter", "athena-numeric", "athena"],
    why: "athena-gc is below IR / VM / engine",
  },
  {
    crate: "projects/athena-types/Cargo.toml",
    forbid: [
      "athena-gc",
      "athena-numeric",
      "athena-ir",
      "athena-rewriter",
      "athena-vm",
      "athena-engine",
      "athena",
    ],
    why: "athena-types is the bottom identity crate",
  },
  {
    crate: "projects/athena-ir/Cargo.toml",
    forbid: ["athena-vm", "athena-engine", "athena-rewriter", "athena"],
    why: "athena-ir must not reverse-depend on VM / engine",
  },
  {
    crate: "projects/athena-rewriter/Cargo.toml",
    forbid: ["athena-vm", "athena-engine", "athena"],
    why: "athena-rewriter must not reverse-depend on VM / engine",
  },
  {
    crate: "projects/athena-numeric/Cargo.toml",
    forbid: ["athena-ir", "athena-rewriter", "athena-vm", "athena-engine", "athena"],
    why: "athena-numeric must not reverse-depend on IR / VM / engine",
  },
];

const failures = [];

for (const rule of rules) {
  const abs = path.join(root, rule.crate);
  if (!fs.existsSync(abs)) {
    failures.push(`${rule.crate}: missing Cargo.toml`);
    continue;
  }
  const deps = depNames(readToml(rule.crate));
  for (const bad of rule.forbid) {
    if (deps.has(bad)) {
      failures.push(`${rule.crate}: forbidden dependency \`${bad}\` (${rule.why})`);
    }
  }
}

// athena-engine must depend on athena-vm (engine above runtime).
const engineDeps = depNames(readToml("projects/athena-engine/Cargo.toml"));
if (!engineDeps.has("athena-vm")) {
  failures.push("projects/athena-engine/Cargo.toml: missing required dependency `athena-vm`");
}

// Public surface regression: athena-vm lib.rs must not re-export domain/planning types.
const vmLibText = fs.readFileSync(path.join(root, "projects/athena-vm/src/lib.rs"), "utf8");
const bannedExports = [
  "DomainGoal",
  "Claim",
  "Proof",
  "ProviderRegistry",
  "PolynomialStore",
  "MatrixStore",
  "FrontierStore",
  "AdmissionGate",
  "TermStore",
];
for (const name of bannedExports) {
  const reUse = new RegExp(`\\bpub\\s+use\\b[^;]*\\b${name}\\b`);
  const reDef = new RegExp(`\\bpub\\s+(?:struct|enum|type|trait)\\s+${name}\\b`);
  if (reUse.test(vmLibText) || reDef.test(vmLibText)) {
    failures.push(`projects/athena-vm/src/lib.rs: banned public symbol \`${name}\``);
  }
}

if (failures.length) {
  console.error("Crate dependency gate failed:\n" + failures.map((f) => "  - " + f).join("\n"));
  process.exit(1);
}
console.log("Crate dependency gate OK");
