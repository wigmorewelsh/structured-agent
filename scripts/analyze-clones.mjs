#!/usr/bin/env node
import { readFileSync } from "fs";

const reportPath = process.argv[2] ?? "/tmp/jscpd-out/jscpd-report.json";
const minCluster = parseInt(process.argv[3] ?? "3", 10);

const report = JSON.parse(readFileSync(reportPath, "utf8"));

const OVERLAP_TOLERANCE = 5;

function fragKey(file, start, end) {
  return `${file}:${start}:${end}`;
}

const fragments = new Map();

function register(file, start, end) {
  const k = fragKey(file, start, end);
  if (!fragments.has(k)) fragments.set(k, { file, start, end });
  return k;
}

for (const clone of report.duplicates) {
  register(clone.firstFile.name, clone.firstFile.start, clone.firstFile.end);
  register(clone.secondFile.name, clone.secondFile.start, clone.secondFile.end);
}

const byFile = new Map();
for (const [k, frag] of fragments) {
  if (!byFile.has(frag.file)) byFile.set(frag.file, []);
  byFile.get(frag.file).push(k);
}

const keyToLocation = new Map();
const locationFragments = new Map();
let nextLocId = 0;

for (const keys of byFile.values()) {
  keys.sort((a, b) => fragments.get(a).start - fragments.get(b).start);

  const groups = [];
  for (const k of keys) {
    const frag = fragments.get(k);
    let placed = false;
    for (const group of groups) {
      if (frag.start <= group.end + OVERLAP_TOLERANCE) {
        group.keys.push(k);
        group.end = Math.max(group.end, frag.end);
        placed = true;
        break;
      }
    }
    if (!placed) {
      groups.push({ keys: [k], start: frag.start, end: frag.end });
    }
  }

  for (const group of groups) {
    const locId = nextLocId++;
    const rep = fragments.get(group.keys[0]);
    locationFragments.set(locId, {
      file: rep.file,
      start: group.start,
      end: group.end,
    });
    for (const k of group.keys) {
      keyToLocation.set(k, locId);
    }
  }
}

const locParent = new Map();

function locFind(l) {
  if (!locParent.has(l)) locParent.set(l, l);
  if (locParent.get(l) !== l) locParent.set(l, locFind(locParent.get(l)));
  return locParent.get(l);
}

function locUnion(a, b) {
  const ra = locFind(a),
    rb = locFind(b);
  if (ra !== rb) locParent.set(ra, rb);
}

for (const clone of report.duplicates) {
  const ka = fragKey(
    clone.firstFile.name,
    clone.firstFile.start,
    clone.firstFile.end,
  );
  const kb = fragKey(
    clone.secondFile.name,
    clone.secondFile.start,
    clone.secondFile.end,
  );
  const la = keyToLocation.get(ka);
  const lb = keyToLocation.get(kb);
  if (la !== undefined && lb !== undefined) locUnion(la, lb);
}

const locClusters = new Map();
for (const locId of locationFragments.keys()) {
  const root = locFind(locId);
  if (!locClusters.has(root)) locClusters.set(root, []);
  locClusters.get(root).push(locId);
}

const results = [];
for (const members of locClusters.values()) {
  if (members.length >= minCluster) {
    results.push(
      members
        .map((id) => locationFragments.get(id))
        .sort((a, b) => a.file.localeCompare(b.file) || a.start - b.start),
    );
  }
}

results.sort((a, b) => b.length - a.length);

for (const members of results) {
  const lines = members.map((m) => m.end - m.start + 1);
  const avgLines = Math.round(lines.reduce((a, b) => a + b, 0) / lines.length);
  console.log(
    `\nCluster: ${members.length} instances (~${avgLines} lines each)`,
  );
  for (const m of members) {
    const short = m.file.replace(/^.*\/src\//, "src/");
    console.log(`  ${short}  [${m.start}-${m.end}]`);
  }
}

if (results.length === 0) {
  console.log(`No clusters of ${minCluster}+ found.`);
}
