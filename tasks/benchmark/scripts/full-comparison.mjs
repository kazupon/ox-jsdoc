/**
 * Full benchmark comparison across all parsers and pipelines.
 *
 * Compares:
 * - comment-parser (JS)
 * - @es-joy/jsdoccomment (JS, wraps comment-parser)
 * - ox-jsdoc napi (Rust via napi)
 * - ox-jsdoc napi with parseTypes (Rust via napi, type parsing enabled)
 * - jsdoc-type-pratt-parser (JS, type parser only)
 * - ox-jsdoc parseType / parseCheck (Rust type parser only via napi)
 */

import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { bench, group, run } from "mitata";
import { parseSync } from "oxc-parser";
import { parse as commentParserParse } from "comment-parser";
import { parseComment as jsdoccommentParse } from "@es-joy/jsdoccomment";
import { parse as oxParse, parseType as oxParseType, parseTypeCheck as oxParseTypeCheck } from "ox-jsdoc";
import { parse as jtpParse } from "jsdoc-type-pratt-parser";

// Load wasm version
const wasmPath = path.resolve(
  fileURLToPath(import.meta.url),
  "../../../../wasm/ox-jsdoc/src-js/index.js",
);
const wasmModule = await import(wasmPath);
const wasmBinary = await readFile(
  path.resolve(
    fileURLToPath(import.meta.url),
    "../../../../wasm/ox-jsdoc/pkg/ox_jsdoc_wasm_bg.wasm",
  ),
);
await wasmModule.initWasm(wasmBinary);
const wasmParse = wasmModule.parse;
const wasmParseType = wasmModule.parseType;
const wasmParseTypeCheck = wasmModule.parseTypeCheck;

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "../../..");
const fixturesRoot = path.join(repoRoot, "fixtures", "perf");

// ============================================================================
// Load fixtures
// ============================================================================

const buckets = [
  "common",
  "description-heavy",
  "type-heavy",
  "special-tag",
  "malformed",
  "source",
  "toolchain",
];

const fixtures = await loadFixtures();
const allCommentTexts = fixtures.flatMap((f) => f.commentTexts);

console.log(`Loaded ${fixtures.length} fixtures, ${allCommentTexts.length} JSDoc blocks total\n`);

// ============================================================================
// 1. Comment Parser Comparison (per fixture)
// ============================================================================

group("comment parsing — per fixture", () => {
  for (const fixture of fixtures) {
    const label = `${fixture.bucket}/${fixture.name}`;

    bench(`comment-parser: ${label}`, () => {
      for (const text of fixture.commentTexts) {
        commentParserParse(text);
      }
    });

    bench(`jsdoccomment: ${label}`, () => {
      for (const text of fixture.commentTexts) {
        try { jsdoccommentParse(text); } catch {}
      }
    });

    bench(`ox-jsdoc napi: ${label}`, () => {
      for (const text of fixture.commentTexts) {
        oxParse(text);
      }
    });

    bench(`ox-jsdoc wasm: ${label}`, () => {
      for (const text of fixture.commentTexts) {
        wasmParse(text);
      }
    });
  }
});

// ============================================================================
// 2. Comment Parser Comparison (all fixtures batch)
// ============================================================================

group("comment parsing — all fixtures batch", () => {
  bench(`comment-parser (${allCommentTexts.length} blocks)`, () => {
    for (const text of allCommentTexts) {
      commentParserParse(text);
    }
  });

  bench(`jsdoccomment (${allCommentTexts.length} blocks)`, () => {
    for (const text of allCommentTexts) {
      try { jsdoccommentParse(text); } catch {}
    }
  });

  bench(`ox-jsdoc napi (${allCommentTexts.length} blocks)`, () => {
    for (const text of allCommentTexts) {
      oxParse(text);
    }
  });

  bench(`ox-jsdoc napi + parseTypes (${allCommentTexts.length} blocks)`, () => {
    for (const text of allCommentTexts) {
      oxParse(text, { parseTypes: true, typeParseMode: "jsdoc" });
    }
  });

  bench(`ox-jsdoc wasm (${allCommentTexts.length} blocks)`, () => {
    for (const text of allCommentTexts) {
      wasmParse(text);
    }
  });

  bench(`ox-jsdoc wasm + parseTypes (${allCommentTexts.length} blocks)`, () => {
    for (const text of allCommentTexts) {
      wasmParse(text, { parseTypes: true, typeParseMode: "jsdoc" });
    }
  });
});

// ============================================================================
// 3. Type Parser Comparison
// ============================================================================

const TYPE_EXPRESSIONS = [
  "string", "number", "boolean", "null", "undefined", "*", "?",
  "string | number",
  "string | number | boolean",
  "string | number | null | undefined | boolean",
  "A & B", "A & B & C",
  "Array<string>", "Map<string, number>",
  "Map<string, Array<number>>",
  "Object.<string, number>",
  "string[]", "number[][]",
  "?string", "string?", "!Object", "string=", "...string",
  "(x: number) => string",
  "{}", "{key: string}", "{a: string, b: number, c: boolean}",
  "keyof MyType", "typeof myVar",
  "x is string", "asserts x is T",
  "readonly string[]", "unique symbol",
  "[string, number]",
  "Array<string> | Map<string, number> | null",
  '"success" | "error" | "pending"',
];

group("type parser — batch", () => {
  bench(`ox-jsdoc napi parseCheck (${TYPE_EXPRESSIONS.length} types)`, () => {
    for (const expr of TYPE_EXPRESSIONS) {
      oxParseTypeCheck(expr, "typescript");
    }
  });

  bench(`ox-jsdoc napi parseType (${TYPE_EXPRESSIONS.length} types)`, () => {
    for (const expr of TYPE_EXPRESSIONS) {
      oxParseType(expr, "typescript");
    }
  });

  bench(`ox-jsdoc wasm parseTypeCheck (${TYPE_EXPRESSIONS.length} types)`, () => {
    for (const expr of TYPE_EXPRESSIONS) {
      wasmParseTypeCheck(expr, "typescript");
    }
  });

  bench(`ox-jsdoc wasm parseType (${TYPE_EXPRESSIONS.length} types)`, () => {
    for (const expr of TYPE_EXPRESSIONS) {
      wasmParseType(expr, "typescript");
    }
  });

  bench(`jsdoc-type-pratt-parser (${TYPE_EXPRESSIONS.length} types)`, () => {
    for (const expr of TYPE_EXPRESSIONS) {
      try { jtpParse(expr, "typescript"); } catch {}
    }
  });
});

// ============================================================================
// 4. Type Parser — Individual expressions
// ============================================================================

const INDIVIDUAL_TYPES = [
  "string",
  "string | number",
  "string | number | boolean",
  "Array<string>",
  "{a: string, b: number}",
];

group("type parser — individual", () => {
  for (const expr of INDIVIDUAL_TYPES) {
    bench(`ox-jsdoc napi parseCheck: ${expr}`, () => {
      oxParseTypeCheck(expr, "typescript");
    });
    bench(`ox-jsdoc wasm parseTypeCheck: ${expr}`, () => {
      wasmParseTypeCheck(expr, "typescript");
    });
    bench(`jsdoc-type-pratt-parser: ${expr}`, () => {
      try { jtpParse(expr, "typescript"); } catch {}
    });
  }
});

// ============================================================================
// 5. ox-jsdoc parse_types on vs off
// ============================================================================

group("ox-jsdoc — parseTypes on vs off (all fixtures)", () => {
  bench("napi parseTypes: false", () => {
    for (const text of allCommentTexts) {
      oxParse(text);
    }
  });

  bench("napi parseTypes: true (jsdoc)", () => {
    for (const text of allCommentTexts) {
      oxParse(text, { parseTypes: true, typeParseMode: "jsdoc" });
    }
  });

  bench("wasm parseTypes: false", () => {
    for (const text of allCommentTexts) {
      wasmParse(text);
    }
  });

  bench("wasm parseTypes: true (jsdoc)", () => {
    for (const text of allCommentTexts) {
      wasmParse(text, { parseTypes: true, typeParseMode: "jsdoc" });
    }
  });
});

// ============================================================================
// Run
// ============================================================================

await run();

// ============================================================================
// Helpers
// ============================================================================

async function loadFixtures() {
  const allFixtures = [];
  for (const bucket of buckets) {
    const bucketDir = path.join(fixturesRoot, bucket);
    let entries;
    try { entries = await readdir(bucketDir, { withFileTypes: true }); } catch { continue; }
    for (const entry of entries) {
      if (!entry.isFile() || !isSupportedFixture(entry.name)) continue;
      const filePath = path.join(bucketDir, entry.name);
      const sourceText = await readFile(filePath, "utf8");
      const commentTexts = entry.name.endsWith(".jsdoc")
        ? [sourceText.trimEnd()]
        : extractJsdocBlocks(filePath, sourceText);
      if (commentTexts.length === 0) continue;
      allFixtures.push({
        bucket,
        name: entry.name.replace(/\.(?:jsdoc|[cm]?[jt]sx?)$/, ""),
        commentTexts,
      });
    }
  }
  return allFixtures;
}

function isSupportedFixture(name) {
  return /\.(?:jsdoc|[cm]?[jt]sx?)$/.test(name);
}

function extractJsdocBlocks(filePath, sourceText) {
  const result = parseSync(filePath, sourceText);
  return result.comments
    .filter((c) => c.type === "Block" && c.value.startsWith("*"))
    .map((c) => sourceText.slice(c.start, c.end));
}
