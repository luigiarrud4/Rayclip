import { test } from "node:test";
import assert from "node:assert/strict";

const src = new URL("../src/lib/format.ts", import.meta.url).href;
const { groupEntries, fold, shortPath, matchesQuery, matchedOnlyInOcr } = await import(src);

const now = Date.now();
const DAY = 86400000;

function entry(id, createdAt, extra = {}) {
  return {
    id,
    kind: "text",
    title: "",
    text: "x",
    paths: [],
    sizeBytes: 1,
    createdAt,
    pinned: false,
    hash: id,
    ...extra,
  };
}

function labels(rows) {
  return rows.map((b) => b.label + "=" + b.entries.map((e) => e.id).join(","));
}

test("grupos usam os ultimos 7 dias para This Week (nao getDay)", () => {
  const rows = groupEntries([
    entry("p", now - 10 * DAY, { pinned: true }),
    entry("t", now - 1000),
    entry("y", now - 26 * 3600000),
    entry("w", now - 3 * DAY),
    entry("m", now - 40 * DAY),
  ]);
  const got = labels(rows);
  assert.equal(got[0], "Pinned=p");
  assert.ok(got.includes("Today=t"), got.join(" "));
  assert.ok(got.includes("Yesterday=y"), got.join(" "));
  assert.ok(got.includes("This Week=w"), got.join(" "));
  assert.ok(!got.includes("undefined"), got.join(" "));
});

test("pin nao aparece duas vezes", () => {
  const rows = groupEntries([entry("a", now, { pinned: true }), entry("b", now)]);
  assert.equal(rows.length, 2);
  assert.deepEqual(rows[0].entries.map((e) => e.id), ["a"]);
  assert.deepEqual(rows[1].entries.map((e) => e.id), ["b"]);
});

test("lista vazia nao gera bucket", () => {
  assert.deepEqual(groupEntries([]), []);
});

test("busca ignora acento e caixa", () => {
  assert.equal(fold("Café São Paulo"), "cafe sao paulo");
  const e = entry("1", now, { text: "Café" });
  assert.ok(matchesQuery(e, "cafe"));
  assert.ok(matchesQuery(e, "CAFE"));
  assert.ok(!matchesQuery(e, "cha"));
  assert.ok(matchesQuery(e, "  "));
});

test("acerto so no OCR e sinalizado", () => {
  const e = entry("1", now, { text: "nota", ocrText: "R$ 1.234,56" });
  assert.ok(matchesQuery(e, "1.234"));
  assert.ok(matchedOnlyInOcr(e, "1.234"));
  assert.ok(!matchedOnlyInOcr(e, "nota"));
});

test("shortPath mantem o nome do arquivo", () => {
  assert.equal(shortPath("C:\\Users\\Lugia\\Documents\\relatorio.docx"), "…/Documents/relatorio.docx");
  assert.equal(shortPath("C:\\a.txt"), "C:\\a.txt");
  assert.equal(shortPath(null), "—");
});
