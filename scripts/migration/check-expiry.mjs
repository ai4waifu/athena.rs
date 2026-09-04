#!/usr/bin/env node
/**
 * Migration expiry gate.
 * - expired: any match fails
 * - expiring: match count above baseline_max_matches fails
 * - before expiry: emit owner / replacement warnings for remaining matches
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "../..");
const manifestPath = path.join(__dirname, "expiry.json");

function walk(dir, globs, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    const rel = path.relative(root, full).split(path.sep).join("/");
    if (entry.isDirectory()) {
      if (entry.name === "target" || entry.name === "node_modules" || entry.name === ".git") continue;
      walk(full, globs, out);
      continue;
    }
    if (!entry.isFile()) continue;
    if (!globs.some((g) => matchGlob(rel, g))) continue;
    out.push(full);
  }
  return out;
}

function matchGlob(rel, glob) {
  // Minimal glob: ** and * only.
  const escaped = glob
    .replace(/[.+^${}()|[\]\\]/g, "\\$&")
    .replace(/\*\*/g, "{{GLOBSTAR}}")
    .replace(/\*/g, "[^/]*")
    .replace(/{{GLOBSTAR}}/g, ".*");
  return new RegExp(`^${escaped}$`).test(rel);
}

function countMatches(files, pattern) {
  const re = new RegExp(pattern, "g");
  let total = 0;
  const hits = [];
  for (const file of files) {
    const text = fs.readFileSync(file, "utf8");
    const m = text.match(re);
    if (!m) continue;
    total += m.length;
    hits.push(`${path.relative(root, file).split(path.sep).join("/")}:${m.length}`);
  }
  return { total, hits };
}

function todayUtc() {
  return new Date().toISOString().slice(0, 10);
}

const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
let failed = false;
const today = todayUtc();

for (const entry of manifest.entries) {
  const files = [];
  for (const g of entry.globs) {
    // Collect from repo root with each glob independently.
    walk(root, [g], files);
  }
  const unique = [...new Set(files)];
  const { total, hits } = countMatches(unique, entry.pattern);
  const expiredByDate = today >= entry.expiry;
  const effectiveStatus = expiredByDate && entry.status === "expiring" ? "expired" : entry.status;

  const header = `[${effectiveStatus}] ${entry.id} owner=${entry.owner} replacement=${entry.replacement} expiry=${entry.expiry} matches=${total}`;
  if (total > 0) {
    console.warn(header);
    for (const h of hits.slice(0, 20)) console.warn(`  ${h}`);
    if (hits.length > 20) console.warn(`  ... ${hits.length - 20} more files`);
  } else {
    console.log(header);
  }

  if (effectiveStatus === "expired") {
    if (total > 0) {
      console.error(`FAIL expired symbol still present: ${entry.id}`);
      failed = true;
    }
  } else if (effectiveStatus === "expiring") {
    if (total > entry.baseline_max_matches) {
      console.error(
        `FAIL ${entry.id}: matches ${total} exceed baseline_max_matches ${entry.baseline_max_matches} (new references banned)`,
      );
      failed = true;
    }
  }
}

if (failed) {
  process.exit(1);
}
console.log("expiry check passed");
