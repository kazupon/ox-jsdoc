/**
 * Type parser benchmark: ox-jsdoc (napi) vs jsdoc-type-pratt-parser.
 *
 * Compares parse performance for the same type expressions using both parsers.
 */

import { bench, group, run } from "mitata";
import { parse as oxParse, parseType as oxParseType, parseTypeCheck as oxParseTypeCheck } from "ox-jsdoc";
import { parse as jtpParse } from "jsdoc-type-pratt-parser";

const TYPE_EXPRESSIONS = [
  // Basic types
  "string",
  "number",
  "boolean",
  "null",
  "undefined",
  "*",
  "?",

  // Union
  "string | number",
  "string | number | boolean",
  "string | number | null | undefined | boolean",

  // Intersection
  "A & B",
  "A & B & C",

  // Generic
  "Array<string>",
  "Map<string, number>",
  "Map<string, Array<number>>",
  "Promise<Map<string, Array<number>>>",
  "Object.<string, number>",

  // Array shorthand
  "string[]",
  "number[][]",

  // Modifiers
  "?string",
  "string?",
  "!Object",
  "string=",
  "...string",

  // Function (arrow only — closure-style function() doesn't work in typescript mode for jtp)
  "(x: number) => string",
  "(a: string, b: number, c: boolean) => void",

  // Object
  "{}",
  "{key: string}",
  "{a: string, b: number, c: boolean}",
  "{a?: string, b?: number}",

  // TypeScript-specific (conditional not supported by jtp in typescript mode)
  // "T extends U ? X : Y",
  "keyof MyType",
  "typeof myVar",
  "import('./module')",
  "x is string",
  "asserts x is T",
  "readonly string[]",
  "unique symbol",

  // Tuple
  "[string, number]",
  "[a: string, b: number]",

  // Complex real-world
  "Array<string> | Map<string, number> | null",
  // "K extends keyof T ? T[K] : never",
  '"success" | "error" | "pending"',
  "Record<string, Array<number>>",
];

// --- Individual type expression benchmarks (parse only, no stringify) ---
group("parse only — no stringify (fairest comparison)", () => {
  for (const typeExpr of TYPE_EXPRESSIONS.slice(0, 10)) {
    bench(`ox-jsdoc parseCheck: ${typeExpr}`, () => {
      oxParseTypeCheck(typeExpr, "typescript");
    });
    bench(`ox-jsdoc parseType: ${typeExpr}`, () => {
      oxParseType(typeExpr, "typescript");
    });
    bench(`jsdoc-type-pratt-parser: ${typeExpr}`, () => {
      try { jtpParse(typeExpr, "typescript"); } catch {}
    });
  }
});

// --- Batch benchmark: parse all types ---
group("batch — parse only (fairest comparison)", () => {
  bench(`ox-jsdoc parseCheck (${TYPE_EXPRESSIONS.length} types)`, () => {
    for (const typeExpr of TYPE_EXPRESSIONS) {
      oxParseTypeCheck(typeExpr, "typescript");
    }
  });

  bench(`ox-jsdoc parseType (${TYPE_EXPRESSIONS.length} types)`, () => {
    for (const typeExpr of TYPE_EXPRESSIONS) {
      oxParseType(typeExpr, "typescript");
    }
  });

  bench(`jsdoc-type-pratt-parser (${TYPE_EXPRESSIONS.length} types)`, () => {
    for (const typeExpr of TYPE_EXPRESSIONS) {
      try { jtpParse(typeExpr, "typescript"); } catch {}
    }
  });
});

group("batch — full pipeline (comment + type parse)", () => {
  const comments = TYPE_EXPRESSIONS.map(
    (expr) => `/** @param {${expr}} x */`,
  );

  bench(`ox-jsdoc parse (${TYPE_EXPRESSIONS.length} types)`, () => {
    for (const comment of comments) {
      oxParse(comment, { parseTypes: true, typeParseMode: "typescript" });
    }
  });

  bench(`jsdoc-type-pratt-parser (${TYPE_EXPRESSIONS.length} types)`, () => {
    for (const typeExpr of TYPE_EXPRESSIONS) {
      try { jtpParse(typeExpr, "typescript"); } catch {}
    }
  });
});

// --- Type-only benchmark (no comment parsing overhead) ---
group("type-only parse (jsdoc-type-pratt-parser baseline)", () => {
  const simpleTypes = [
    "string",
    "string | number",
    "Array<string>",
    "{a: string, b: number}",
  ];

  for (const typeExpr of simpleTypes) {
    bench(`jtp: ${typeExpr}`, () => {
      jtpParse(typeExpr, "typescript");
    });
  }
});

// --- ox-jsdoc: parse_types enabled vs disabled ---
group("ox-jsdoc: parse_types on vs off", () => {
  const comments = TYPE_EXPRESSIONS.map(
    (expr) => `/** @param {${expr}} x */`,
  );

  bench("parse_types: true", () => {
    for (const comment of comments) {
      oxParse(comment, { parseTypes: true, typeParseMode: "typescript" });
    }
  });

  bench("parse_types: false", () => {
    for (const comment of comments) {
      oxParse(comment);
    }
  });
});

// --- Mode comparison ---
group("ox-jsdoc: mode comparison", () => {
  const typeExpr = "Array.<string> | Map.<string, number>";
  const comment = `/** @param {${typeExpr}} x */`;

  for (const mode of ["jsdoc", "closure", "typescript"]) {
    bench(`mode: ${mode}`, () => {
      oxParse(comment, { parseTypes: true, typeParseMode: mode });
    });
  }
});

await run();
