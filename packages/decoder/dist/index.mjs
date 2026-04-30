//#region src/internal/constants.ts
const PAYLOAD_MASK = 1073741823;
//#endregion
//#region src/internal/helpers.ts
/**
* Low-level helpers shared by every Remote* class.
*
* Mirrors `crates/ox_jsdoc_binary/src/decoder/helpers.rs`.
*
* @author kazuya kawaguchi (a.k.a. kazupon)
* @license MIT
*/
/**
* Read a 4-byte aligned u32 from the source file's `Uint32Array` view.
*
* 5–10× faster than `DataView.getUint32` because the typed-array element
* load compiles to a single CPU instruction in V8's TurboFan, whereas
* `getUint32` goes through a runtime stub. Caller MUST guarantee
* `byteOffset` is 4-byte aligned (writer pads section boundaries to keep
* every node record's u32 fields aligned).
*/
function readU32Aligned(sourceFile, byteOffset) {
	return sourceFile.uint32View[byteOffset >>> 2];
}
/**
* Resolve the Extended Data byte offset for a node.
*
* Throws if the node's TypeTag is not `Extended` (matches the Rust
* `debug_assert!`). Used by classes whose Extended Data carries the
* Children bitmask + per-kind fields.
*/
function extOffsetOf(internal) {
	const { byteIndex, sourceFile } = internal;
	const nodeData = readU32Aligned(sourceFile, byteIndex + 12);
	const typeTag = nodeData >>> 30 & 3;
	if (typeTag !== 2) throw new Error(`Node at index ${internal.index} is not Extended type (got tag 0b${typeTag.toString(2)})`);
	return sourceFile.extendedDataOffset + (nodeData & PAYLOAD_MASK);
}
/**
* Read the 30-bit String payload of a string-leaf node, dispatching on the
* 2-bit TypeTag:
*
* - `TypeTag::String` (`0b01`): payload is a String Offsets table index;
*   resolves via `getString`. Returns `null` when the payload equals the
*   None sentinel.
* - `TypeTag::StringInline` (`0b11`): payload is a packed `(offset, length)`
*   pair pointing directly into String Data. Resolves via
*   `getStringByOffsetAndLength` (zero-copy slice, no Offsets-table hop).
*/
function stringPayloadOf(internal) {
	const { byteIndex, sourceFile } = internal;
	const nodeData = readU32Aligned(sourceFile, byteIndex + 12);
	const tag = nodeData >>> 30 & 3;
	const payload = nodeData & PAYLOAD_MASK;
	if (tag === 3) {
		const length = payload & 255;
		const offset = payload >>> 8;
		return sourceFile.getStringByOffsetAndLength(offset, length);
	}
	if (payload === 1073741823) return null;
	return sourceFile.getString(payload);
}
/**
* Resolve the leading `StringField` (6 bytes at offset 0 of the record)
* of an Extended-type node whose record begins with a StringField slot
* (Pattern 3 TypeNodes such as `TypeKeyValue.key`, `TypeMethodSignature.name`,
* `TypeSymbol.value`).
*
* Returns `""` when the field equals the NONE sentinel.
*/
function extStringLeaf(internal) {
	return extStringFieldRequired(internal, 0);
}
/**
* Read the 30-bit Children bitmask payload of a Children-type node.
*/
function childrenBitmaskPayloadOf(internal) {
	const { byteIndex, sourceFile } = internal;
	return readU32Aligned(sourceFile, byteIndex + 12) & PAYLOAD_MASK;
}
/**
* Read the `next_sibling` field for the given node index.
*/
function readNextSibling(sourceFile, nodeIndex) {
	return readU32Aligned(sourceFile, sourceFile.nodesOffset + nodeIndex * 24 + 20);
}
/**
* Return the first child of the parent at `parentIndex` (= `parentIndex + 1`
* if its `parent_index` field equals `parentIndex`). Returns `0` when the
* parent has no child.
*/
function firstChildIndex(sourceFile, parentIndex) {
	const candidate = parentIndex + 1;
	if (candidate >= sourceFile.nodeCount) return 0;
	if (readU32Aligned(sourceFile, sourceFile.nodesOffset + candidate * 24 + 16) !== parentIndex) return 0;
	return candidate;
}
/**
* Find the `visitorIndex`-th set bit in `bitmask` and return the
* corresponding child node index. Returns `0` when the slot is unset
* or the sibling chain is truncated.
*/
function childIndexAtVisitorIndex(internal, bitmask, visitorIndex) {
	if ((bitmask & 1 << visitorIndex) === 0) return 0;
	const skip = popcount(bitmask & (1 << visitorIndex) - 1);
	let child = internal.index + 1;
	for (let i = 0; i < skip; i++) {
		const next = readNextSibling(internal.sourceFile, child);
		if (next === 0) return 0;
		child = next;
	}
	return child;
}
/**
* Build a Remote* instance for the child at `visitorIndex` under the parent
* described by `internal`. Reads the parent's bitmask from Extended Data
* (so the parent must be Extended type).
*/
function childNodeAtVisitorIndex(internal, visitorIndex) {
	const childIdx = childIndexAtVisitorIndex(internal, internal.view.getUint8(extOffsetOf(internal)), visitorIndex);
	if (childIdx === 0) return null;
	return internal.sourceFile.getNode(childIdx, thisNode(internal), internal.rootIndex);
}
/**
* Same as `childNodeAtVisitorIndex` but reads the bitmask from the 30-bit
* Node Data payload (Children-type parents).
*/
function childNodeAtVisitorIndexChildren(internal, visitorIndex) {
	const childIdx = childIndexAtVisitorIndex(internal, childrenBitmaskPayloadOf(internal) & 255, visitorIndex);
	if (childIdx === 0) return null;
	return internal.sourceFile.getNode(childIdx, thisNode(internal), internal.rootIndex);
}
/**
* Resolve an Optional `StringField` slot at `fieldOffset` inside this
* node's Extended Data record (`null` when the slot equals the NONE
* sentinel).
*
* The 6-byte slot is read as `(offset: u32 LE, length: u16 LE)` and then
* passed to `RemoteSourceFile.getStringByField`.
*/
function extStringField(internal, fieldOffset) {
	const ext = extOffsetOf(internal) + fieldOffset;
	const offset = internal.view.getUint32(ext, true);
	const length = internal.view.getUint16(ext + 4, true);
	return internal.sourceFile.getStringByField(offset, length);
}
/**
* Resolve a Required `StringField` slot at `fieldOffset` (returns `""` for
* the NONE sentinel).
*/
function extStringFieldRequired(internal, fieldOffset) {
	const ext = extOffsetOf(internal) + fieldOffset;
	const offset = internal.view.getUint32(ext, true);
	const length = internal.view.getUint16(ext + 4, true);
	return internal.sourceFile.getStringByField(offset, length) ?? "";
}
/**
* Read a little-endian u32 at `fieldOffset` inside this node's Extended Data.
*
* Used by compat-mode tail fields (line indices). Caller is responsible for
* gating reads on `sourceFile.compatMode` since basic-mode ED records do not
* reserve these bytes.
*/
function extU32(internal, fieldOffset) {
	return internal.view.getUint32(extOffsetOf(internal) + fieldOffset, true);
}
/**
* Read a u8 at `fieldOffset` inside this node's Extended Data.
*/
function extU8(internal, fieldOffset) {
	return internal.view.getUint8(extOffsetOf(internal) + fieldOffset);
}
/**
* Compute the absolute `[start, end]` range of a node by adding the root's
* `base_offset` to the relative Pos/End fields.
*/
function absoluteRange(internal) {
	const { byteIndex, rootIndex, sourceFile } = internal;
	const pos = readU32Aligned(sourceFile, byteIndex + 4);
	const end = readU32Aligned(sourceFile, byteIndex + 8);
	const baseOffset = sourceFile.getRootBaseOffset(rootIndex);
	return [baseOffset + pos, baseOffset + end];
}
/**
* Look up the lazy node instance described by `internal` (used as the
* `parent` argument when constructing children). Goes through the
* sourceFile's nodeCache to keep instances stable.
*/
function thisNode(internal) {
	return internal.sourceFile.getNode(internal.index, internal.parent, internal.rootIndex);
}
/**
* Population count for a u8.
*/
function popcount(byte) {
	let n = byte & 255;
	n -= n >> 1 & 85;
	n = (n & 51) + (n >> 2 & 51);
	return n + (n >> 4) & 15;
}
//#endregion
//#region src/internal/inspect.ts
/**
* Shared `Symbol.for('nodejs.util.inspect.custom')` helper.
*
* Returning a plain object whose prototype is set to an empty named class
* makes `console.log(node)` print the class label (e.g. `JsdocBlock { ... }`)
* in Node-family runtimes. Same trick as oxc raw transfer.
*
* In browsers `Symbol.for('nodejs.util.inspect.custom')` is harmless (the
* key just becomes another property on the object).
*
* @author kazuya kawaguchi (a.k.a. kazupon)
* @license MIT
*/
const inspectSymbol = Symbol.for("nodejs.util.inspect.custom");
/**
* Cache of empty named classes used as the inspect prototype, keyed by
* type name. The class is created lazily on first access so unused types
* don't pollute the runtime.
*/
const debugClassCache = /* @__PURE__ */ new Map();
/**
* Get (or create) the empty class whose name matches `typeName` so that
* `console.log(node)` shows `TypeName { ... }` instead of `Object { ... }`.
*/
function debugClass(typeName) {
	const cached = debugClassCache.get(typeName);
	if (cached !== void 0) return cached;
	const cls = new Function(`return class ${typeName} {}`)();
	debugClassCache.set(typeName, cls);
	return cls;
}
/**
* Build the inspect-payload from a plain JSON object — moves it under
* the `typeName`-labelled prototype so Node prints the right class name.
*/
function inspectPayload(jsonObj, typeName) {
	return Object.setPrototypeOf(jsonObj, debugClass(typeName).prototype);
}
//#endregion
//#region src/internal/node-list.ts
/**
* `RemoteNodeList` — Array-compatible view over a parent's per-list metadata
* slot.
*
* As of the NodeList-wrapper-elimination format change, every variable-length
* child list is represented as an inline `(head_index: u32, count: u16)` pair
* stored at a known per-Kind byte offset inside the parent's Extended Data
* block. The decoder reads `head` and walks the `next_sibling` chain exactly
* `count` times. Empty arrays share `EMPTY_NODE_LIST` to avoid per-call
* allocation.
*
* @author kazuya kawaguchi (a.k.a. kazupon)
* @license MIT
*/
/**
* `Array` subclass returned by every "node list" getter. Inheriting from
* `Array` gives us `length` / `map` / `filter` / `forEach` etc. for free;
* indexed access (`list[i]`) returns lazy class instances built up front.
*/
var RemoteNodeList = class extends Array {};
/**
* Empty singleton — every "no children" getter returns this so callers can
* branch on `length === 0` without allocating.
*/
const EMPTY_NODE_LIST = new RemoteNodeList();
/**
* Build a `RemoteNodeList` from the per-list metadata slot at byte offset
* `slotOffset` inside the parent's Extended Data block. Reads
* `(head_index: u32, count: u16)` and walks `count` siblings starting from
* `head_index`.
*
* Mirrors `decoder::helpers::read_list_metadata` + `NodeListIter::new` on
* the Rust side.
*/
function nodeListAtSlotExtended(internal, slotOffset) {
	const ext = extOffsetOf(internal) + slotOffset;
	const head = internal.view.getUint32(ext, true);
	const count = internal.view.getUint16(ext + 4, true);
	if (head === 0 || count === 0) return EMPTY_NODE_LIST;
	return collectNodeListChildren(internal, head, count);
}
/**
* Walk `count` siblings starting at `headIndex` and collect them into a
* `RemoteNodeList`. The parent of every collected child is `internal`.
*/
function collectNodeListChildren(parentInternal, headIndex, count) {
	const { sourceFile, rootIndex } = parentInternal;
	const list = new RemoteNodeList();
	const parent = thisNode(parentInternal);
	let cursor = headIndex;
	for (let i = 0; i < count && cursor !== 0; i++) {
		const child = sourceFile.getNode(cursor, parent, rootIndex);
		if (child !== null) list.push(child);
		cursor = readNextSibling(sourceFile, cursor);
	}
	return list;
}
//#endregion
//#region src/internal/preserve-whitespace.ts
/**
* Description-text post-processing helpers.
*
* JS port of `crates/ox_jsdoc/src/parser/text.rs::parsed_preserving_whitespace`
* — kept byte-for-byte equivalent so `RemoteJsdocBlock.descriptionText(true)`
* (Binary AST decoder) and `JsdocBlock::description_text(true)` (typed AST
* Rust) produce identical output for any given raw description slice.
*
* See `design/008-oxlint-oxfmt-support/README.md` §3 for the algorithm
* design + §4.3 for the JS API contract this powers.
*
* @author kazuya kawaguchi (a.k.a. kazupon)
* @license MIT
*/
const ALNUM_OR_UNDERSCORE = /^[\p{L}\p{N}_]/u;
/**
* Reflow a raw description slice into preserve-whitespace form:
*
* - Strip the leading `* ` margin from each comment-continuation line
*   (at most ONE space after `*` is consumed; extra indentation is kept
*   so Markdown indented code blocks survive).
* - Preserve blank `*` lines as empty lines (paragraph structure).
* - Preserve markdown emphasis: `*foo*` / `*_bold_*` keep the leading
*   `*` because the character right after is alphanumeric or `_`.
*
* Single-line input takes a fast path that just returns `raw.trim()`.
*
*/
function parsedPreservingWhitespace(raw) {
	if (!raw.includes("\n")) return raw.trim();
	const lines = (raw.endsWith("\n") ? raw.slice(0, -1) : raw).split("\n");
	let result = "";
	for (let i = 0; i < lines.length; i++) {
		if (i > 0) result += "\n";
		const trimmed = lines[i].trim();
		if (trimmed.startsWith("*")) {
			const rest = trimmed.slice(1);
			if (!ALNUM_OR_UNDERSCORE.test(rest)) {
				result += rest.startsWith(" ") ? rest.slice(1) : rest;
				continue;
			}
		}
		result += trimmed;
	}
	return result;
}
//#endregion
//#region src/internal/nodes/jsdoc.ts
/**
* Lazy classes for the 15 comment AST kinds (`0x01 - 0x0F`).
*
* Each class follows the `#internal` pattern from `js-decoder.md`:
* private state lives in a single object so the V8 hidden class stays
* stable across all instances, and lazily constructed children are cached
* inside the same object.
*
* @author kazuya kawaguchi (a.k.a. kazupon)
* @license MIT
*/
/** Size of one `(head: u32, count: u16)` list metadata slot. */
const LIST_METADATA_SIZE = 6;
/** `JsdocBlock.descriptionLines` slot offset (right after 8 StringFields). */
const JSDOC_BLOCK_DESC_LINES_SLOT = 50;
/** `JsdocBlock.tags` slot offset. */
const JSDOC_BLOCK_TAGS_SLOT = JSDOC_BLOCK_DESC_LINES_SLOT + LIST_METADATA_SIZE;
/** `JsdocBlock.inlineTags` slot offset. */
const JSDOC_BLOCK_INLINE_TAGS_SLOT = JSDOC_BLOCK_DESC_LINES_SLOT + 2 * LIST_METADATA_SIZE;
const JSDOC_BLOCK_END_LINE_OFFSET = JSDOC_BLOCK_DESC_LINES_SLOT + 3 * LIST_METADATA_SIZE + 2;
const JSDOC_BLOCK_DESC_START_LINE_OFFSET = JSDOC_BLOCK_END_LINE_OFFSET + 4;
const JSDOC_BLOCK_DESC_END_LINE_OFFSET = JSDOC_BLOCK_DESC_START_LINE_OFFSET + 4;
const JSDOC_BLOCK_LAST_DESC_LINE_OFFSET = JSDOC_BLOCK_DESC_END_LINE_OFFSET + 4;
const JSDOC_BLOCK_HAS_PRETERMINAL_DESC_OFFSET = JSDOC_BLOCK_LAST_DESC_LINE_OFFSET + 4;
const JSDOC_BLOCK_HAS_PRETERMINAL_TAG_DESC_OFFSET = JSDOC_BLOCK_HAS_PRETERMINAL_DESC_OFFSET + 1;
/** `0xFFFFFFFF` sentinel for `Option<u32>` line indices in compat mode. */
const COMPAT_LINE_NONE = 4294967295;
/** `0xFF` sentinel for `Option<u8>` flags in compat mode. */
const COMPAT_U8_NONE = 255;
/** `JsdocTag.typeLines` slot offset (right after 3 StringFields). */
const JSDOC_TAG_TYPE_LINES_SLOT = 20;
/** `JsdocTag.descriptionLines` slot offset. */
const JSDOC_TAG_DESC_LINES_SLOT = JSDOC_TAG_TYPE_LINES_SLOT + LIST_METADATA_SIZE;
/** `JsdocTag.inlineTags` slot offset. */
const JSDOC_TAG_INLINE_TAGS_SLOT = JSDOC_TAG_TYPE_LINES_SLOT + 2 * LIST_METADATA_SIZE;
const JSDOC_TAG_COMPAT_DELIMITER = JSDOC_TAG_TYPE_LINES_SLOT + 3 * LIST_METADATA_SIZE;
const JSDOC_TAG_COMPAT_POST_DELIMITER = JSDOC_TAG_COMPAT_DELIMITER + 6;
const JSDOC_TAG_COMPAT_POST_TAG = JSDOC_TAG_COMPAT_POST_DELIMITER + 6;
const JSDOC_TAG_COMPAT_POST_TYPE = JSDOC_TAG_COMPAT_POST_TAG + 6;
const JSDOC_TAG_COMPAT_POST_NAME = JSDOC_TAG_COMPAT_POST_TYPE + 6;
const JSDOC_TAG_COMPAT_INITIAL = JSDOC_TAG_COMPAT_POST_NAME + 6;
const JSDOC_TAG_COMPAT_LINE_END = JSDOC_TAG_COMPAT_INITIAL + 6;
const COMPAT_LINE_DELIMITER = 6;
const COMPAT_LINE_POST_DELIMITER = 12;
const COMPAT_LINE_INITIAL = 18;
/** Empty `tokens` template — every key present so consumers can index by
* field name without truthy checks. Mirrors comment-parser's
* `seedTokens()`. */
function emptyTokens() {
	return {
		start: "",
		delimiter: "",
		postDelimiter: "",
		tag: "",
		postTag: "",
		name: "",
		postName: "",
		type: "",
		postType: "",
		description: "",
		end: "",
		lineEnd: ""
	};
}
/** Concatenate every token field (in jsdoccomment order) to rebuild the
* `source` string for one line. Mirrors `comment-parser.stringify()` for
* a single Line. */
function tokensToSource(t) {
	return t.start + t.delimiter + t.postDelimiter + t.tag + t.postTag + t.type + t.postType + t.name + t.postName + t.description + t.end + t.lineEnd;
}
const DEFAULT_NO_TYPES = new Set([
	"default",
	"defaultvalue",
	"description",
	"example",
	"file",
	"fileoverview",
	"license",
	"overview",
	"see",
	"summary"
]);
const DEFAULT_NO_NAMES = new Set([
	"access",
	"author",
	"default",
	"defaultvalue",
	"description",
	"example",
	"exception",
	"file",
	"fileoverview",
	"kind",
	"license",
	"overview",
	"return",
	"returns",
	"since",
	"summary",
	"throws",
	"version",
	"variation"
]);
const TAG_HEADER_RE = /^@(?<tag>[^\s{]+)(?<postTag>\s*)/u;
function consumeBalancedType(text) {
	if (!text.startsWith("{")) return null;
	let depth = 0;
	for (let i = 0; i < text.length; i++) {
		const ch = text[i];
		if (ch === "{") depth++;
		if (ch === "}") {
			depth--;
			if (depth === 0) {
				const type = text.slice(0, i + 1);
				const rest = text.slice(i + 1);
				const ws = rest.match(/^\s*/u)?.[0] ?? "";
				return {
					type,
					postType: ws,
					rest: rest.slice(ws.length)
				};
			}
		}
	}
	return null;
}
function braceDepthDelta(text, initialDepth = 0) {
	let depth = initialDepth;
	let closeIndex = -1;
	for (let i = 0; i < text.length; i++) {
		const ch = text[i];
		if (ch === "{") depth++;
		if (ch === "}") {
			depth--;
			if (depth === 0) {
				closeIndex = i;
				break;
			}
		}
	}
	return {
		depth,
		closeIndex
	};
}
function splitName(text) {
	if (text === "") return {
		name: "",
		postName: "",
		rest: ""
	};
	const match = text.match(/^(?<name>\S+)(?<postName>\s*)(?<rest>.*)$/su);
	return {
		name: match?.groups?.name ?? "",
		postName: match?.groups?.postName ?? "",
		rest: match?.groups?.rest ?? ""
	};
}
function splitTemplateName(text) {
	let pos;
	if (text.startsWith("[") && text.includes("]")) {
		const endingBracketPos = text.lastIndexOf("]");
		pos = text.slice(endingBracketPos).search(/(?<![\s,])\s/u);
		if (pos > -1) pos += endingBracketPos;
	} else pos = text.search(/(?<![\s,])\s/u);
	const name = pos === -1 ? text : text.slice(0, pos);
	const match = (pos === -1 ? "" : text.slice(pos)).match(/^(?<postName>\s*)(?<rest>[^\r]*)(?<lineEnd>\r)?$/u);
	return {
		name,
		postName: match?.groups?.postName ?? "",
		rest: match?.groups?.rest ?? ""
	};
}
function applyTagTokens(tokens) {
	const header = tokens.description.match(TAG_HEADER_RE);
	if (!header?.groups) return;
	const tagName = header.groups.tag;
	tokens.tag = "@" + tagName;
	tokens.postTag = header.groups.postTag ?? "";
	let rest = tokens.description.slice(header[0].length);
	tokens.description = "";
	if (!DEFAULT_NO_TYPES.has(tagName)) {
		const parsedType = consumeBalancedType(rest);
		if (parsedType) {
			tokens.type = parsedType.type;
			tokens.postType = parsedType.postType;
			rest = parsedType.rest;
		} else if (rest.startsWith("{")) {
			tokens.type = rest;
			return;
		}
	}
	if (tagName === "template") {
		const parsedName = splitTemplateName(rest);
		tokens.name = parsedName.name;
		tokens.postName = parsedName.postName;
		tokens.description = parsedName.rest;
		return;
	}
	if (DEFAULT_NO_NAMES.has(tagName) || tagName === "see" && /\{@link.+?\}/u.test(tokensToSource(tokens) + rest)) {
		tokens.description = rest;
		return;
	}
	const parsedName = splitName(rest);
	tokens.name = parsedName.name;
	tokens.postName = parsedName.postName;
	tokens.description = parsedName.rest;
}
function splitPhysicalLines(sourceText, baseOffset) {
	const lines = [];
	let offset = 0;
	for (const match of sourceText.matchAll(/.*(?:\r\n|\n|\r|$)/gu)) {
		const raw = match[0];
		if (raw === "") break;
		let lineEnd = "";
		let source = raw;
		if (raw.endsWith("\r\n")) {
			lineEnd = "\r\n";
			source = raw.slice(0, -2);
		} else if (raw.endsWith("\n") || raw.endsWith("\r")) {
			lineEnd = raw.at(-1) ?? "";
			source = raw.slice(0, -1);
		}
		lines.push({
			source,
			lineEnd,
			startOffset: baseOffset + offset,
			endOffset: baseOffset + offset + source.length
		});
		offset += raw.length;
	}
	return lines;
}
function lineToSourceEntry(line, number) {
	const tokens = emptyTokens();
	tokens.lineEnd = "";
	let rest = line.source;
	const opening = rest.indexOf("/**");
	if (opening !== -1) {
		tokens.start = rest.slice(0, opening);
		tokens.delimiter = "/**";
		rest = rest.slice(opening + 3);
	} else {
		const initial = rest.match(/^\s*/u)?.[0] ?? "";
		tokens.start = initial;
		rest = rest.slice(initial.length);
		if (rest.startsWith("*") && !rest.startsWith("*/")) {
			tokens.delimiter = "*";
			rest = rest.slice(1);
		}
	}
	if (tokens.delimiter) {
		const postDelimiter = rest.match(/^[ \t]*/u)?.[0] ?? "";
		tokens.postDelimiter = postDelimiter;
		rest = rest.slice(postDelimiter.length);
	}
	if (rest.endsWith("*/")) {
		tokens.end = "*/";
		rest = rest.slice(0, -2);
	}
	tokens.description = rest;
	applyTagTokens(tokens);
	return {
		number,
		source: tokensToSource(tokens),
		tokens,
		startOffset: line.startOffset,
		endOffset: line.endOffset
	};
}
function applyMultilineTypeTokens(entries) {
	let depth = 0;
	for (const entry of entries) {
		const { tokens } = entry;
		if (depth === 0) {
			if (!tokens.tag) continue;
			const typeText = tokens.type || tokens.description;
			if (!typeText.startsWith("{")) continue;
			const typeState = braceDepthDelta(typeText);
			if (typeState.closeIndex !== -1) continue;
			tokens.type = typeText;
			tokens.description = "";
			depth = typeState.depth;
			entry.source = tokensToSource(tokens);
			continue;
		}
		let text = tokens.description;
		if (tokens.delimiter && tokens.postDelimiter.length > 1) {
			text = tokens.postDelimiter.slice(1) + text;
			tokens.postDelimiter = tokens.postDelimiter[0] ?? "";
		}
		const typeState = braceDepthDelta(text, depth);
		if (typeState.closeIndex === -1) {
			tokens.type = text;
			tokens.description = "";
			depth = typeState.depth;
			entry.source = tokensToSource(tokens);
			continue;
		}
		tokens.type = text.slice(0, typeState.closeIndex + 1);
		const rest = text.slice(typeState.closeIndex + 1);
		const postType = rest.match(/^\s*/u)?.[0] ?? "";
		tokens.postType = postType;
		const parsedName = splitName(rest.slice(postType.length));
		tokens.name = parsedName.name;
		tokens.postName = parsedName.postName;
		tokens.description = parsedName.rest;
		depth = 0;
		entry.source = tokensToSource(tokens);
	}
}
function stripSourceEntryMeta(entry, number = entry.number) {
	return {
		number,
		source: entry.source,
		tokens: entry.tokens
	};
}
function buildSourceEntriesForNode(node) {
	const baseOffset = node.sourceFile.getRootBaseOffset(node.rootIndex);
	const [rootStart, rootEnd] = node.parent !== null ? node.parent.range : node.range;
	const entries = splitPhysicalLines(node.sourceFile.sliceSourceText(node.rootIndex, rootStart - baseOffset, rootEnd - baseOffset) ?? "", rootStart).map((line, number) => lineToSourceEntry(line, number));
	applyMultilineTypeTokens(entries);
	return entries;
}
function buildBlockSource(block) {
	return buildSourceEntriesForNode(block).map((entry) => stripSourceEntryMeta(entry));
}
function buildTagSourceFromRoot(tag) {
	const entries = buildSourceEntriesForNode(tag);
	const [tagStart, tagEnd] = tag.range;
	return entries.filter((entry) => entry.endOffset > tagStart && entry.startOffset < tagEnd).map((entry) => stripSourceEntryMeta(entry));
}
function stripTypeBraces(type) {
	return type.replace(/^\{/u, "").replace(/\}$/u, "");
}
function compactDescriptionFromEntries(entries) {
	return entries.filter((entry) => entry.tokens.tag === "").map((entry) => entry.tokens.description).map((description) => description.replace(/^\s*/u, "")).filter(Boolean).join(" ");
}
/** Read the 6-bit Common Data byte for a node. */
function commonData$1(internal) {
	return internal.view.getUint8(internal.byteIndex + 1) & 63;
}
/**
* `JsdocInlineTagFormat` numeric → string label.
* Mirrors Rust's `JsdocInlineTagFormat` enum order.
*/
const INLINE_TAG_FORMATS = [
	"plain",
	"pipe",
	"space",
	"prefix",
	"unknown"
];
/**
* `JsdocBlock` (Kind 0x01) — root of every parsed `/** ... *​/` comment.
*/
var RemoteJsdocBlock = class {
	type = "JsdocBlock";
	#internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this.#internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile,
			$range: void 0,
			$description: void 0,
			$delimiter: void 0,
			$postDelimiter: void 0,
			$terminal: void 0,
			$lineEnd: void 0,
			$initial: void 0,
			$delimiterLineBreak: void 0,
			$preterminalLineBreak: void 0,
			$descriptionLines: void 0,
			$tags: void 0,
			$inlineTags: void 0
		};
	}
	get range() {
		const internal = this.#internal;
		return internal.$range !== void 0 ? internal.$range : internal.$range = absoluteRange(internal);
	}
	get parent() {
		return this.#internal.parent;
	}
	get sourceFile() {
		return this.#internal.sourceFile;
	}
	get rootIndex() {
		return this.#internal.rootIndex;
	}
	/** Top-level description string (`null` when absent). The
	* `emptyStringForNull` option only affects `toJSON()` output. */
	get description() {
		const internal = this.#internal;
		const cached = internal.$description;
		if (cached !== void 0) return cached;
		return internal.$description = extStringField(internal, 2);
	}
	/** Source-preserving `*` line-prefix delimiter. */
	get delimiter() {
		const internal = this.#internal;
		const cached = internal.$delimiter;
		if (cached !== void 0) return cached;
		return internal.$delimiter = extStringFieldRequired(internal, 8);
	}
	/** Source-preserving space after `*`. */
	get postDelimiter() {
		const internal = this.#internal;
		const cached = internal.$postDelimiter;
		if (cached !== void 0) return cached;
		return internal.$postDelimiter = extStringFieldRequired(internal, 14);
	}
	/** Source-preserving `*​/` terminal. */
	get terminal() {
		const internal = this.#internal;
		const cached = internal.$terminal;
		if (cached !== void 0) return cached;
		return internal.$terminal = extStringFieldRequired(internal, 20);
	}
	/** Source-preserving line-end characters. */
	get lineEnd() {
		const internal = this.#internal;
		const cached = internal.$lineEnd;
		if (cached !== void 0) return cached;
		return internal.$lineEnd = extStringFieldRequired(internal, 26);
	}
	/** Indentation before the leading `*`. */
	get initial() {
		const internal = this.#internal;
		const cached = internal.$initial;
		if (cached !== void 0) return cached;
		return internal.$initial = extStringFieldRequired(internal, 32);
	}
	/** Line-break right after `/**`. */
	get delimiterLineBreak() {
		const internal = this.#internal;
		const cached = internal.$delimiterLineBreak;
		if (cached !== void 0) return cached;
		return internal.$delimiterLineBreak = extStringFieldRequired(internal, 38);
	}
	/** Line-break right before `*​/`. */
	get preterminalLineBreak() {
		const internal = this.#internal;
		const cached = internal.$preterminalLineBreak;
		if (cached !== void 0) return cached;
		return internal.$preterminalLineBreak = extStringFieldRequired(internal, 44);
	}
	/** Total number of LogicalLines in this comment (compat-mode only). */
	get endLine() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		return extU32(internal, JSDOC_BLOCK_END_LINE_OFFSET);
	}
	/** Index of the first description line, or `null` when absent. */
	get descriptionStartLine() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		const v = extU32(internal, JSDOC_BLOCK_DESC_START_LINE_OFFSET);
		return v === COMPAT_LINE_NONE ? null : v;
	}
	/** Index of the last description line, or `null` when absent. */
	get descriptionEndLine() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		const v = extU32(internal, JSDOC_BLOCK_DESC_END_LINE_OFFSET);
		return v === COMPAT_LINE_NONE ? null : v;
	}
	/** Description-boundary index (jsdoccomment's `lastDescriptionLine` —
	* actually the index of the first tag/end line). `null` when absent. */
	get lastDescriptionLine() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		const v = extU32(internal, JSDOC_BLOCK_LAST_DESC_LINE_OFFSET);
		return v === COMPAT_LINE_NONE ? null : v;
	}
	/** `1` when block description text exists on the `*​/` line. */
	get hasPreterminalDescription() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		return extU8(internal, JSDOC_BLOCK_HAS_PRETERMINAL_DESC_OFFSET);
	}
	/** `1` when tag description text exists on the `*​/` line; `null` when not
	* applicable (no active lastTag at end). */
	get hasPreterminalTagDescription() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		const v = extU8(internal, JSDOC_BLOCK_HAS_PRETERMINAL_TAG_DESC_OFFSET);
		return v === COMPAT_U8_NONE ? null : v;
	}
	/**
	* Raw description slice (with `*` prefix and blank lines intact).
	* Returns `null` when the buffer was not parsed with
	* `preserveWhitespace: true` (the per-node
	* `has_description_raw_span` Common Data bit is clear), or when the
	* block has no description.
	*
	* Phase 5 layout: the span sits at the **last 8 bytes** of the ED
	* record (offset = `compatMode ? 90 : 68` = the basic / compat ED size).
	* See `design/008-oxlint-oxfmt-support/README.md` §4.2 / §4.3.
	*/
	get descriptionRaw() {
		const internal = this.#internal;
		if ((commonData$1(internal) & 1) === 0) return null;
		const spanOff = internal.sourceFile.compatMode ? 90 : 68;
		const start = extU32(internal, spanOff);
		const end = extU32(internal, spanOff + 4);
		return internal.sourceFile.sliceSourceText(internal.rootIndex, start, end);
	}
	/**
	* Description text. When `preserveWhitespace` is `true`, blank lines
	* and indentation past the `* ` prefix are preserved (algorithm: see
	* `parsedPreservingWhitespace` / design §3). When `false` or omitted,
	* returns the compact view (`description` getter).
	*
	* Returns `null` when no description is present, or when
	* `preserveWhitespace=true` is requested on a buffer that wasn't
	* parsed with the matching `preserveWhitespace: true` parse option.
	*/
	descriptionText(preserveWhitespace) {
		if (preserveWhitespace) {
			const raw = this.descriptionRaw;
			return raw === null ? null : parsedPreservingWhitespace(raw);
		}
		return this.description;
	}
	/** Top-level description lines. */
	get descriptionLines() {
		const internal = this.#internal;
		const cached = internal.$descriptionLines;
		if (cached !== void 0) return cached;
		return internal.$descriptionLines = nodeListAtSlotExtended(internal, JSDOC_BLOCK_DESC_LINES_SLOT);
	}
	/** Block tags. */
	get tags() {
		const internal = this.#internal;
		const cached = internal.$tags;
		if (cached !== void 0) return cached;
		return internal.$tags = nodeListAtSlotExtended(internal, JSDOC_BLOCK_TAGS_SLOT);
	}
	/** Inline tags found inside the top-level description. */
	get inlineTags() {
		const internal = this.#internal;
		const cached = internal.$inlineTags;
		if (cached !== void 0) return cached;
		return internal.$inlineTags = nodeListAtSlotExtended(internal, JSDOC_BLOCK_INLINE_TAGS_SLOT);
	}
	toJSON() {
		const internal = this.#internal;
		const nullToEmpty = internal.sourceFile.emptyStringForNull;
		const json = {
			type: this.type,
			range: [...this.range],
			description: nullToEmpty ? this.description ?? "" : this.description,
			delimiter: this.delimiter,
			postDelimiter: this.postDelimiter,
			terminal: this.terminal,
			lineEnd: this.lineEnd,
			initial: this.initial,
			delimiterLineBreak: this.delimiterLineBreak,
			preterminalLineBreak: this.preterminalLineBreak,
			descriptionLines: this.descriptionLines.map((n) => n.toJSON()),
			tags: this.tags.map((n) => n.toJSON()),
			inlineTags: this.inlineTags.map((n) => n.toJSON())
		};
		if (internal.sourceFile.compatMode) {
			const source = buildBlockSource(this);
			const sourceDescription = compactDescriptionFromEntries(source);
			if (sourceDescription) json.description = sourceDescription;
			json.endLine = this.endLine;
			const dsl = this.descriptionStartLine;
			if (dsl !== null) json.descriptionStartLine = dsl;
			const del = this.descriptionEndLine;
			if (del !== null) json.descriptionEndLine = del;
			const ldl = this.lastDescriptionLine;
			if (ldl !== null) json.lastDescriptionLine = ldl;
			json.hasPreterminalDescription = this.hasPreterminalDescription;
			const hptd = this.hasPreterminalTagDescription;
			if (hptd !== null) json.hasPreterminalTagDescription = hptd;
			const raw = this.descriptionRaw;
			if (raw !== null) json.descriptionRaw = raw;
			json.source = source;
			json.tags = json.tags.filter((tag) => {
				const firstTokens = tag.source?.[0]?.tokens;
				return Boolean(firstTokens?.tag);
			});
		}
		return json;
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "JsdocBlock");
	}
};
/**
* `JsdocDescriptionLine` (Kind 0x02). Both basic and compat modes store
* `description` as the leading StringField of the Extended Data record.
*/
var RemoteJsdocDescriptionLine = class {
	type = "JsdocDescriptionLine";
	#internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this.#internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile,
			$range: void 0,
			$description: void 0
		};
	}
	get range() {
		const internal = this.#internal;
		return internal.$range !== void 0 ? internal.$range : internal.$range = absoluteRange(internal);
	}
	get parent() {
		return this.#internal.parent;
	}
	get sourceFile() {
		return this.#internal.sourceFile;
	}
	get rootIndex() {
		return this.#internal.rootIndex;
	}
	/** Description content. Basic mode reads the String payload (Node Data);
	* compat mode reads byte 0-5 of the Extended Data record. */
	get description() {
		const internal = this.#internal;
		const cached = internal.$description;
		if (cached !== void 0) return cached;
		return internal.$description = internal.sourceFile.compatMode ? extStringFieldRequired(internal, 0) : stringPayloadOf(internal) ?? "";
	}
	/** Source-preserving `*` line-prefix (compat-mode only; `null` otherwise). */
	get delimiter() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		return extStringField(internal, COMPAT_LINE_DELIMITER);
	}
	/** Source-preserving space after `*` (compat-mode only). */
	get postDelimiter() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		return extStringField(internal, COMPAT_LINE_POST_DELIMITER);
	}
	/** Indentation before the leading `*` (compat-mode only). */
	get initial() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		return extStringField(internal, COMPAT_LINE_INITIAL);
	}
	toJSON() {
		const json = {
			type: this.type,
			range: [...this.range],
			description: this.description
		};
		if (this.#internal.sourceFile.compatMode) {
			json.delimiter = this.delimiter ?? "";
			json.postDelimiter = this.postDelimiter ?? "";
			json.initial = this.initial ?? "";
		}
		return json;
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "JsdocDescriptionLine");
	}
};
/**
* `JsdocTag` (Kind 0x03) — one block tag (e.g. `@param`).
*/
var RemoteJsdocTag = class {
	type = "JsdocTag";
	#internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this.#internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile,
			$range: void 0,
			$defaultValue: void 0,
			$description: void 0,
			$rawBody: void 0,
			$tag: void 0,
			$rawType: void 0,
			$name: void 0,
			$parsedType: void 0,
			$body: void 0,
			$typeLines: void 0,
			$descriptionLines: void 0,
			$inlineTags: void 0
		};
	}
	get range() {
		const internal = this.#internal;
		return internal.$range !== void 0 ? internal.$range : internal.$range = absoluteRange(internal);
	}
	get parent() {
		return this.#internal.parent;
	}
	get sourceFile() {
		return this.#internal.sourceFile;
	}
	get rootIndex() {
		return this.#internal.rootIndex;
	}
	/** `bit0` of Common Data — was the tag wrapped in `[...]`? */
	get optional() {
		return (commonData$1(this.#internal) & 1) !== 0;
	}
	/** Default value parsed from `[id=foo]` syntax. */
	get defaultValue() {
		const internal = this.#internal;
		const cached = internal.$defaultValue;
		if (cached !== void 0) return cached;
		return internal.$defaultValue = extStringField(internal, 2);
	}
	/** Joined description text. */
	get description() {
		const internal = this.#internal;
		const cached = internal.$description;
		if (cached !== void 0) return cached;
		return internal.$description = extStringField(internal, 8);
	}
	/**
	* Raw description slice (with `*` prefix and blank lines intact).
	* Returns `null` when the buffer was not parsed with
	* `preserveWhitespace: true` (the per-node
	* `has_description_raw_span` Common Data bit is clear), or when the
	* tag has no description.
	*
	* Phase 5 layout: the span sits at the **last 8 bytes** of the ED
	* record (offset = `compatMode ? 80 : 38` = the basic / compat ED size).
	* See `design/008-oxlint-oxfmt-support/README.md` §4.2 / §4.3.
	*/
	get descriptionRaw() {
		const internal = this.#internal;
		if ((commonData$1(internal) & 2) === 0) return null;
		const spanOff = internal.sourceFile.compatMode ? 80 : 38;
		const start = extU32(internal, spanOff);
		const end = extU32(internal, spanOff + 4);
		return internal.sourceFile.sliceSourceText(internal.rootIndex, start, end);
	}
	/**
	* Description text. Identical contract to
	* `RemoteJsdocBlock.descriptionText`.
	*/
	descriptionText(preserveWhitespace) {
		if (preserveWhitespace) {
			const raw = this.descriptionRaw;
			return raw === null ? null : parsedPreservingWhitespace(raw);
		}
		return this.description;
	}
	/** Raw body when the tag uses the `Raw` body variant. */
	get rawBody() {
		const internal = this.#internal;
		const cached = internal.$rawBody;
		if (cached !== void 0) return cached;
		return internal.$rawBody = extStringField(internal, 14);
	}
	/** Mandatory tag-name child (visitor index 0 — the `@name` token). */
	get tag() {
		const internal = this.#internal;
		const cached = internal.$tag;
		if (cached !== void 0) return cached;
		return internal.$tag = childNodeAtVisitorIndex(internal, 0);
	}
	/** Raw `{...}` type source (visitor index 1). */
	get rawType() {
		const internal = this.#internal;
		const cached = internal.$rawType;
		if (cached !== void 0) return cached;
		return internal.$rawType = childNodeAtVisitorIndex(internal, 1);
	}
	/** Tag-name value (visitor index 2). */
	get name() {
		const internal = this.#internal;
		const cached = internal.$name;
		if (cached !== void 0) return cached;
		return internal.$name = childNodeAtVisitorIndex(internal, 2);
	}
	/** `parsedType` child (visitor index 3) — any TypeNode variant. */
	get parsedType() {
		const internal = this.#internal;
		const cached = internal.$parsedType;
		if (cached !== void 0) return cached;
		return internal.$parsedType = childNodeAtVisitorIndex(internal, 3);
	}
	/** Body child (visitor index 4) — Generic / Borrows / Raw variant. */
	get body() {
		const internal = this.#internal;
		const cached = internal.$body;
		if (cached !== void 0) return cached;
		return internal.$body = childNodeAtVisitorIndex(internal, 4);
	}
	/** Source-preserving type lines. */
	get typeLines() {
		const internal = this.#internal;
		const cached = internal.$typeLines;
		if (cached !== void 0) return cached;
		return internal.$typeLines = nodeListAtSlotExtended(internal, JSDOC_TAG_TYPE_LINES_SLOT);
	}
	/** Source-preserving description lines. */
	get descriptionLines() {
		const internal = this.#internal;
		const cached = internal.$descriptionLines;
		if (cached !== void 0) return cached;
		return internal.$descriptionLines = nodeListAtSlotExtended(internal, JSDOC_TAG_DESC_LINES_SLOT);
	}
	/** Inline tags found in this tag's description. */
	get inlineTags() {
		const internal = this.#internal;
		const cached = internal.$inlineTags;
		if (cached !== void 0) return cached;
		return internal.$inlineTags = nodeListAtSlotExtended(internal, JSDOC_TAG_INLINE_TAGS_SLOT);
	}
	/** Source-preserving `*` line-prefix (compat-mode only). */
	get delimiter() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_DELIMITER);
	}
	/** Source-preserving space after `*` (compat-mode only). */
	get postDelimiter() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_POST_DELIMITER);
	}
	/** Whitespace after the `@name` token (compat-mode only). */
	get postTag() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_POST_TAG);
	}
	/** Whitespace after the `{type}` source (compat-mode only). */
	get postType() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_POST_TYPE);
	}
	/** Whitespace after the name token (compat-mode only). */
	get postName() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_POST_NAME);
	}
	/** Indentation before the line's `*` (compat-mode only). */
	get initial() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_INITIAL);
	}
	/** Line ending of the tag's first line (compat-mode only). */
	get lineEnd() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_LINE_END);
	}
	toJSON() {
		const internal = this.#internal;
		const compat = internal.sourceFile.compatMode;
		const nullToEmpty = internal.sourceFile.emptyStringForNull;
		if (compat) {
			const source = buildTagSourceFromRoot(this);
			const headTokens = source[0]?.tokens ?? emptyTokens();
			const tagName = (this.tag?.toJSON() ?? null)?.value ?? "";
			const rawTypeNode = this.rawType?.toJSON() ?? null;
			const hasSource = source.length > 0;
			const rawTypeRaw = hasSource ? stripTypeBraces(headTokens.type) : rawTypeNode?.raw ?? null;
			const nameNode = this.name?.toJSON() ?? null;
			const nameValue = hasSource ? headTokens.name : nameNode?.raw ?? null;
			const description = hasSource ? headTokens.description : this.description;
			const json = {
				type: this.type,
				range: [...this.range],
				tag: tagName,
				rawType: nullToEmpty ? rawTypeRaw ?? "" : rawTypeRaw,
				name: nullToEmpty ? nameValue ?? "" : nameValue,
				description: description ?? (nullToEmpty ? "" : null),
				delimiter: this.delimiter,
				postDelimiter: this.postDelimiter,
				postTag: this.postTag,
				postType: this.postType,
				postName: this.postName,
				initial: this.initial,
				lineEnd: this.lineEnd,
				parsedType: this.parsedType?.toJSON() ?? null,
				typeLines: this.typeLines.map((n) => n.toJSON()),
				descriptionLines: this.descriptionLines.map((n) => n.toJSON()),
				inlineTags: this.inlineTags.map((n) => n.toJSON()),
				source
			};
			const raw = this.descriptionRaw;
			if (raw !== null) json.descriptionRaw = raw;
			return json;
		}
		return {
			type: this.type,
			range: [...this.range],
			optional: this.optional,
			defaultValue: this.defaultValue,
			description: this.description,
			rawBody: this.rawBody,
			tag: this.tag?.toJSON() ?? null,
			rawType: this.rawType?.toJSON() ?? null,
			name: this.name?.toJSON() ?? null,
			parsedType: this.parsedType?.toJSON() ?? null,
			body: this.body?.toJSON() ?? null,
			typeLines: this.typeLines.map((n) => n.toJSON()),
			descriptionLines: this.descriptionLines.map((n) => n.toJSON()),
			inlineTags: this.inlineTags.map((n) => n.toJSON())
		};
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "JsdocTag");
	}
};
/**
* Build a class for a single-string-leaf node. Captures `accessorName` so
* the resolved value is exposed under the right property name (`value`,
* `raw`, or `name`) per the Rust enum's variant.
*/
function defineStringLeaf(typeName, accessorName) {
	return class {
		type = typeName;
		_internal;
		constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
			this._internal = {
				view,
				byteIndex,
				index,
				rootIndex,
				parent,
				sourceFile,
				$range: void 0,
				$value: void 0
			};
			const internal = this._internal;
			Object.defineProperty(this, accessorName, {
				get() {
					const cached = internal.$value;
					if (cached !== void 0) return cached;
					return internal.$value = stringPayloadOf(internal) ?? "";
				},
				enumerable: true,
				configurable: false
			});
		}
		get range() {
			const internal = this._internal;
			return internal.$range !== void 0 ? internal.$range : internal.$range = absoluteRange(internal);
		}
		get parent() {
			return this._internal.parent;
		}
		toJSON() {
			const value = this[accessorName] ?? "";
			return {
				type: this.type,
				range: [...this.range],
				[accessorName]: value
			};
		}
		[inspectSymbol]() {
			return inspectPayload(this.toJSON(), typeName);
		}
	};
}
/** `JsdocTagName` (Kind 0x04) — the `@name` token text. */
var RemoteJsdocTagName = class extends defineStringLeaf("JsdocTagName", "value") {};
/** `JsdocTagNameValue` (Kind 0x05) — value after the type in `@param`. */
var RemoteJsdocTagNameValue = class extends defineStringLeaf("JsdocTagNameValue", "raw") {};
/** `JsdocTypeSource` (Kind 0x06) — raw `{...}` text inside a tag. */
var RemoteJsdocTypeSource = class extends defineStringLeaf("JsdocTypeSource", "raw") {};
/** `JsdocRawTagBody` (Kind 0x0B) — raw text body fallback. */
var RemoteJsdocRawTagBody = class extends defineStringLeaf("JsdocRawTagBody", "raw") {};
/** `JsdocNamepathSource` (Kind 0x0D) — namepath token. */
var RemoteJsdocNamepathSource = class extends defineStringLeaf("JsdocNamepathSource", "raw") {};
/** `JsdocIdentifier` (Kind 0x0E) — bare identifier. */
var RemoteJsdocIdentifier = class extends defineStringLeaf("JsdocIdentifier", "name") {};
/** `JsdocText` (Kind 0x0F) — raw text. */
var RemoteJsdocText = class extends defineStringLeaf("JsdocText", "value") {};
/**
* `JsdocTypeLine` (Kind 0x07).
*/
var RemoteJsdocTypeLine = class {
	type = "JsdocTypeLine";
	#internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this.#internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile,
			$range: void 0,
			$rawType: void 0
		};
	}
	get range() {
		const internal = this.#internal;
		return internal.$range !== void 0 ? internal.$range : internal.$range = absoluteRange(internal);
	}
	get parent() {
		return this.#internal.parent;
	}
	/** Raw `{...}` line content. Basic mode reads the String payload;
	* compat mode reads byte 0-5 of the Extended Data record. */
	get rawType() {
		const internal = this.#internal;
		const cached = internal.$rawType;
		if (cached !== void 0) return cached;
		return internal.$rawType = internal.sourceFile.compatMode ? extStringFieldRequired(internal, 0) : stringPayloadOf(internal) ?? "";
	}
	/** Source-preserving `*` line-prefix (compat-mode only; `null` otherwise). */
	get delimiter() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		return extStringField(internal, COMPAT_LINE_DELIMITER);
	}
	/** Source-preserving space after `*` (compat-mode only). */
	get postDelimiter() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		return extStringField(internal, COMPAT_LINE_POST_DELIMITER);
	}
	/** Indentation before the leading `*` (compat-mode only). */
	get initial() {
		const internal = this.#internal;
		if (!internal.sourceFile.compatMode) return null;
		return extStringField(internal, COMPAT_LINE_INITIAL);
	}
	toJSON() {
		const json = {
			type: this.type,
			range: [...this.range],
			rawType: this.rawType
		};
		if (this.#internal.sourceFile.compatMode) {
			json.delimiter = this.delimiter ?? "";
			json.postDelimiter = this.postDelimiter ?? "";
			json.initial = this.initial ?? "";
		}
		return json;
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "JsdocTypeLine");
	}
};
/**
* `JsdocInlineTag` (Kind 0x08) — e.g. `{@link Foo}`.
*/
var RemoteJsdocInlineTag = class {
	type = "JsdocInlineTag";
	#internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this.#internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile,
			$range: void 0,
			$namepathOrURL: void 0,
			$text: void 0,
			$rawBody: void 0
		};
	}
	get range() {
		const internal = this.#internal;
		return internal.$range !== void 0 ? internal.$range : internal.$range = absoluteRange(internal);
	}
	get parent() {
		return this.#internal.parent;
	}
	/** Inline tag format string. In compat mode the `'unknown'` variant is
	* mapped to `'plain'` to mirror jsdoccomment's behavior. */
	get format() {
		const internal = this.#internal;
		const raw = INLINE_TAG_FORMATS[commonData$1(internal) & 7] ?? "unknown";
		return raw === "unknown" && internal.sourceFile.compatMode ? "plain" : raw;
	}
	/** Optional name path or URL portion. */
	get namepathOrURL() {
		const internal = this.#internal;
		const cached = internal.$namepathOrURL;
		if (cached !== void 0) return cached;
		return internal.$namepathOrURL = extStringField(internal, 0);
	}
	/** Optional display text portion. */
	get text() {
		const internal = this.#internal;
		const cached = internal.$text;
		if (cached !== void 0) return cached;
		return internal.$text = extStringField(internal, 6);
	}
	/** Raw body text fallback. */
	get rawBody() {
		const internal = this.#internal;
		const cached = internal.$rawBody;
		if (cached !== void 0) return cached;
		return internal.$rawBody = extStringField(internal, 12);
	}
	toJSON() {
		const internal = this.#internal;
		const compat = internal.sourceFile.compatMode;
		const nullToEmpty = internal.sourceFile.emptyStringForNull;
		const json = {
			type: this.type,
			range: [...this.range],
			format: this.format,
			namepathOrURL: nullToEmpty ? this.namepathOrURL ?? "" : this.namepathOrURL,
			text: nullToEmpty ? this.text ?? "" : this.text
		};
		if (!compat) json.rawBody = this.rawBody;
		return json;
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "JsdocInlineTag");
	}
};
/**
* `JsdocGenericTagBody` (Kind 0x09).
*/
var RemoteJsdocGenericTagBody = class {
	type = "JsdocGenericTagBody";
	#internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this.#internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile
		};
	}
	get range() {
		return absoluteRange(this.#internal);
	}
	get parent() {
		return this.#internal.parent;
	}
	/** `true` when the tag separator was `-`. */
	get hasDashSeparator() {
		return (commonData$1(this.#internal) & 1) !== 0;
	}
	/** Description text after the dash separator. */
	get description() {
		return extStringField(this.#internal, 2);
	}
	toJSON() {
		return {
			type: this.type,
			range: [...this.range],
			hasDashSeparator: this.hasDashSeparator,
			description: this.description
		};
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "JsdocGenericTagBody");
	}
};
/**
* `JsdocBorrowsTagBody` (Kind 0x0A) — Children type with `source` + `target`
* children. The child accessors will be filled in once the parser starts
* emitting them; for now the class exposes the standard range/parent/toJSON
* surface.
*/
var RemoteJsdocBorrowsTagBody = class {
	type = "JsdocBorrowsTagBody";
	#internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this.#internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile
		};
	}
	get range() {
		return absoluteRange(this.#internal);
	}
	get parent() {
		return this.#internal.parent;
	}
	toJSON() {
		return {
			type: this.type,
			range: [...this.range]
		};
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "JsdocBorrowsTagBody");
	}
};
/**
* `JsdocParameterName` (Kind 0x0C) — `JsdocTagValue::Parameter` variant.
*/
var RemoteJsdocParameterName = class {
	type = "JsdocParameterName";
	#internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this.#internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile
		};
	}
	get range() {
		return absoluteRange(this.#internal);
	}
	get parent() {
		return this.#internal.parent;
	}
	/** `true` when the parameter was wrapped in `[id]` brackets. */
	get optional() {
		return (commonData$1(this.#internal) & 1) !== 0;
	}
	/** Path text. */
	get path() {
		return extStringFieldRequired(this.#internal, 0);
	}
	/** Default value parsed from `[id=foo]` syntax. */
	get defaultValue() {
		return extStringField(this.#internal, 6);
	}
	toJSON() {
		return {
			type: this.type,
			range: [...this.range],
			optional: this.optional,
			path: this.path,
			defaultValue: this.defaultValue
		};
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "JsdocParameterName");
	}
};
//#endregion
//#region src/internal/nodes/node-list-node.ts
/**
* `RemoteNodeListNode` — wraps the Kind 0x7F NodeList record.
*
* Users almost never construct one directly; the `RemoteNodeList`
* helpers in `node-list.js` walk past the wrapper and expose its children
* directly. This class exists so that `RemoteSourceFile.getNode` can
* still return a stable instance for the wrapper itself (used by some
* helpers when traversing the byte stream).
*
* @author kazuya kawaguchi (a.k.a. kazupon)
* @license MIT
*/
var RemoteNodeListNode = class {
	type = "NodeList";
	_internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this._internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile
		};
	}
	get range() {
		return absoluteRange(this._internal);
	}
	get parent() {
		return this._internal.parent;
	}
	/** Number of elements (stored in the 30-bit Children payload). */
	get elementCount() {
		return childrenBitmaskPayloadOf(this._internal);
	}
	/** Walk and return the wrapper's children as a plain Array. */
	get children() {
		const out = [];
		let cursor = firstChildIndex(this._internal.sourceFile, this._internal.index);
		const parent = thisNode(this._internal);
		while (cursor !== 0) {
			const child = this._internal.sourceFile.getNode(cursor, parent, this._internal.rootIndex);
			if (child !== null) out.push(child);
			cursor = readNextSibling(this._internal.sourceFile, cursor);
		}
		return out;
	}
	toJSON() {
		return {
			type: this.type,
			range: [...this.range],
			elementCount: this.elementCount,
			children: this.children.map((n) => n.toJSON())
		};
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "NodeList");
	}
};
//#endregion
//#region src/internal/nodes/type-nodes.ts
/**
* Lazy classes for the 45 TypeNode kinds (`0x80 - 0xAC`).
*
* Mirrors `crates/ox_jsdoc_binary/src/decoder/nodes/type_node.rs`.
*
* Three structural patterns are at play:
*
* - **Pattern 1 — String only** (5 kinds): payload lives in the 30-bit
*   String slot, optionally with quote/special-type flags in Common Data.
* - **Pattern 2 — Children only** (29 kinds): Children-type Node Data
*   carries the bitmask; child accessors use the Children-type helpers.
* - **Pattern 3 — Mixed** (6 kinds): Extended type with a key/name string
*   plus zero or one child node.
*
* Plus 5 pure leaves (`TypeNull` etc.) using Children-type with zero payload.
*
* @author kazuya kawaguchi (a.k.a. kazupon)
* @license MIT
*/
/**
* Single per-list metadata slot offset for TypeNode parents that own one
* variable-length child list (TypeUnion, TypeIntersection, TypeTuple,
* TypeObject, TypeGeneric, TypeTypeParameter, TypeParameterList). Mirrors
* `crates/ox_jsdoc_binary/src/writer/nodes/type_node.rs::TYPE_LIST_PARENT_SLOT`.
*/
const TYPE_LIST_PARENT_SLOT = 0;
function commonData(internal) {
	return internal.view.getUint8(internal.byteIndex + 1) & 63;
}
/**
* Build a Pattern 1 (string-only) class.
*
* `extraJson` lets variants append per-Kind metadata (quote / special_type)
* to the JSON output without duplicating boilerplate.
*/
function defineStringPattern(typeName, extraJson) {
	return class {
		type = typeName;
		_internal;
		constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
			this._internal = {
				view,
				byteIndex,
				index,
				rootIndex,
				parent,
				sourceFile
			};
		}
		get range() {
			return absoluteRange(this._internal);
		}
		get parent() {
			return this._internal.parent;
		}
		get value() {
			return stringPayloadOf(this._internal) ?? "";
		}
		toJSON() {
			const json = {
				type: this.type,
				range: [...this.range],
				value: this.value
			};
			if (extraJson !== void 0) extraJson(this._internal, json);
			return json;
		}
		[inspectSymbol]() {
			return inspectPayload(this.toJSON(), typeName);
		}
	};
}
/**
* Build a "pure leaf" class (Children-type with zero payload — `TypeNull`,
* `TypeUndefined`, `TypeAny`, `TypeUnknown`, `TypeUniqueSymbol`).
*/
function definePureLeaf(typeName) {
	return class {
		type = typeName;
		_internal;
		constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
			this._internal = {
				view,
				byteIndex,
				index,
				rootIndex,
				parent,
				sourceFile
			};
		}
		get range() {
			return absoluteRange(this._internal);
		}
		get parent() {
			return this._internal.parent;
		}
		toJSON() {
			return {
				type: this.type,
				range: [...this.range]
			};
		}
		[inspectSymbol]() {
			return inspectPayload(this.toJSON(), typeName);
		}
	};
}
var RemoteTypeName = class extends defineStringPattern("TypeName") {};
var RemoteTypeNumber = class extends defineStringPattern("TypeNumber") {};
var RemoteTypeStringValue = class extends defineStringPattern("TypeStringValue", (internal, json) => {
	json.quote = commonData(internal) & 3;
}) {};
var RemoteTypeProperty = class extends defineStringPattern("TypeProperty", (internal, json) => {
	json.quote = commonData(internal) & 3;
}) {};
var RemoteTypeSpecialNamePath = class extends defineStringPattern("TypeSpecialNamePath", (internal, json) => {
	const cd = commonData(internal);
	json.specialType = cd & 3;
	json.quote = cd >> 2 & 3;
}) {};
/** Helper: build a class with one `elements` NodeList child. */
function defineElementsContainer(typeName) {
	return class {
		type = typeName;
		_internal;
		constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
			this._internal = {
				view,
				byteIndex,
				index,
				rootIndex,
				parent,
				sourceFile
			};
		}
		get range() {
			return absoluteRange(this._internal);
		}
		get parent() {
			return this._internal.parent;
		}
		get elements() {
			return nodeListAtSlotExtended(this._internal, TYPE_LIST_PARENT_SLOT);
		}
		toJSON() {
			return {
				type: this.type,
				range: [...this.range],
				elements: this.elements.map((n) => n.toJSON())
			};
		}
		[inspectSymbol]() {
			return inspectPayload(this.toJSON(), typeName);
		}
	};
}
/** Helper: build a class with a single `element` child. */
function defineSingleChildContainer(typeName, extraCommon) {
	return class {
		type = typeName;
		_internal;
		constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
			this._internal = {
				view,
				byteIndex,
				index,
				rootIndex,
				parent,
				sourceFile
			};
		}
		get range() {
			return absoluteRange(this._internal);
		}
		get parent() {
			return this._internal.parent;
		}
		get element() {
			return childNodeAtVisitorIndexChildren(this._internal, 0);
		}
		toJSON() {
			const json = {
				type: this.type,
				range: [...this.range],
				element: this.element?.toJSON() ?? null
			};
			if (extraCommon !== void 0) extraCommon(this._internal, json);
			return json;
		}
		[inspectSymbol]() {
			return inspectPayload(this.toJSON(), typeName);
		}
	};
}
/** Helper: build a class with `left` + `right` children. */
function defineLeftRightContainer(typeName, extraCommon) {
	return class {
		type = typeName;
		_internal;
		constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
			this._internal = {
				view,
				byteIndex,
				index,
				rootIndex,
				parent,
				sourceFile
			};
		}
		get range() {
			return absoluteRange(this._internal);
		}
		get parent() {
			return this._internal.parent;
		}
		get left() {
			return childNodeAtVisitorIndexChildren(this._internal, 0);
		}
		get right() {
			return childNodeAtVisitorIndexChildren(this._internal, 1);
		}
		toJSON() {
			const json = {
				type: this.type,
				range: [...this.range],
				left: this.left?.toJSON() ?? null,
				right: this.right?.toJSON() ?? null
			};
			if (extraCommon !== void 0) extraCommon(this._internal, json);
			return json;
		}
		[inspectSymbol]() {
			return inspectPayload(this.toJSON(), typeName);
		}
	};
}
var RemoteTypeUnion = class extends defineElementsContainer("TypeUnion") {};
var RemoteTypeIntersection = class extends defineElementsContainer("TypeIntersection") {};
var RemoteTypeObject = class {
	type = "TypeObject";
	_internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this._internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile
		};
	}
	get range() {
		return absoluteRange(this._internal);
	}
	get parent() {
		return this._internal.parent;
	}
	get elements() {
		return nodeListAtSlotExtended(this._internal, TYPE_LIST_PARENT_SLOT);
	}
	/** `bits[0:2]` of Common Data — field separator style. */
	get separator() {
		return commonData(this._internal) & 7;
	}
	toJSON() {
		return {
			type: this.type,
			range: [...this.range],
			elements: this.elements.map((n) => n.toJSON()),
			separator: this.separator
		};
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "TypeObject");
	}
};
var RemoteTypeTuple = class extends defineElementsContainer("TypeTuple") {};
var RemoteTypeTypeParameter = class extends defineElementsContainer("TypeTypeParameter") {};
var RemoteTypeParameterList = class extends defineElementsContainer("TypeParameterList") {};
var RemoteTypeParenthesis = class extends defineSingleChildContainer("TypeParenthesis") {};
var RemoteTypeInfer = class extends defineSingleChildContainer("TypeInfer") {};
var RemoteTypeKeyOf = class extends defineSingleChildContainer("TypeKeyOf") {};
var RemoteTypeTypeOf = class extends defineSingleChildContainer("TypeTypeOf") {};
var RemoteTypeImport = class extends defineSingleChildContainer("TypeImport") {};
var RemoteTypeAssertsPlain = class extends defineSingleChildContainer("TypeAssertsPlain") {};
var RemoteTypeReadonlyArray = class extends defineSingleChildContainer("TypeReadonlyArray") {};
var RemoteTypeIndexedAccessIndex = class extends defineSingleChildContainer("TypeIndexedAccessIndex") {};
var RemoteTypeReadonlyProperty = class extends defineSingleChildContainer("TypeReadonlyProperty") {};
/** Modifier types (Nullable / NotNullable / Optional) — single child + position flag. */
function defineModifier(typeName) {
	return defineSingleChildContainer(typeName, (internal, json) => {
		json.position = commonData(internal) & 1;
	});
}
var RemoteTypeNullable = class extends defineModifier("TypeNullable") {};
var RemoteTypeNotNullable = class extends defineModifier("TypeNotNullable") {};
var RemoteTypeOptional = class extends defineModifier("TypeOptional") {};
/** `TypeVariadic` — modifier + extra `square_brackets` flag. */
var RemoteTypeVariadic = class extends defineSingleChildContainer("TypeVariadic", (internal, json) => {
	const cd = commonData(internal);
	json.position = cd & 1;
	json.squareBrackets = (cd & 2) !== 0;
}) {};
var RemoteTypePredicate = class extends defineLeftRightContainer("TypePredicate") {};
var RemoteTypeAsserts = class extends defineLeftRightContainer("TypeAsserts") {};
var RemoteTypeNamePath = class extends defineLeftRightContainer("TypeNamePath", (internal, json) => {
	json.pathType = commonData(internal) & 3;
}) {};
/** `TypeGeneric` — `left` + `elements` NodeList + brackets/dot flags. */
var RemoteTypeGeneric = class {
	type = "TypeGeneric";
	_internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this._internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile
		};
	}
	get range() {
		return absoluteRange(this._internal);
	}
	get parent() {
		return this._internal.parent;
	}
	get brackets() {
		return commonData(this._internal) & 1;
	}
	get dot() {
		return (commonData(this._internal) & 2) !== 0;
	}
	get left() {
		const internal = this._internal;
		const childIdx = firstChildIndex(internal.sourceFile, internal.index);
		if (childIdx === 0) return null;
		return internal.sourceFile.getNode(childIdx, thisNode(internal), internal.rootIndex);
	}
	get elements() {
		return nodeListAtSlotExtended(this._internal, TYPE_LIST_PARENT_SLOT);
	}
	toJSON() {
		return {
			type: this.type,
			range: [...this.range],
			brackets: this.brackets,
			dot: this.dot,
			left: this.left?.toJSON() ?? null,
			elements: this.elements.map((n) => n.toJSON())
		};
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "TypeGeneric");
	}
};
/** `TypeFunction` — parameters + return + type_parameters. */
var RemoteTypeFunction = class {
	type = "TypeFunction";
	_internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this._internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile
		};
	}
	get range() {
		return absoluteRange(this._internal);
	}
	get parent() {
		return this._internal.parent;
	}
	get constructor_() {
		return (commonData(this._internal) & 1) !== 0;
	}
	get arrow() {
		return (commonData(this._internal) & 2) !== 0;
	}
	get parenthesis() {
		return (commonData(this._internal) & 4) !== 0;
	}
	get parameters() {
		return childNodeAtVisitorIndexChildren(this._internal, 0);
	}
	get returnType() {
		return childNodeAtVisitorIndexChildren(this._internal, 1);
	}
	get typeParameters() {
		return childNodeAtVisitorIndexChildren(this._internal, 2);
	}
	toJSON() {
		return {
			type: this.type,
			range: [...this.range],
			constructor: this.constructor_,
			arrow: this.arrow,
			parenthesis: this.parenthesis,
			parameters: this.parameters?.toJSON() ?? null,
			returnType: this.returnType?.toJSON() ?? null,
			typeParameters: this.typeParameters?.toJSON() ?? null
		};
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "TypeFunction");
	}
};
/** `TypeConditional` — check / extends / true / false branches. */
var RemoteTypeConditional = class {
	type = "TypeConditional";
	_internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this._internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile
		};
	}
	get range() {
		return absoluteRange(this._internal);
	}
	get parent() {
		return this._internal.parent;
	}
	get checkType() {
		return childNodeAtVisitorIndexChildren(this._internal, 0);
	}
	get extendsType() {
		return childNodeAtVisitorIndexChildren(this._internal, 1);
	}
	get trueType() {
		return childNodeAtVisitorIndexChildren(this._internal, 2);
	}
	get falseType() {
		return childNodeAtVisitorIndexChildren(this._internal, 3);
	}
	toJSON() {
		return {
			type: this.type,
			range: [...this.range],
			checkType: this.checkType?.toJSON() ?? null,
			extendsType: this.extendsType?.toJSON() ?? null,
			trueType: this.trueType?.toJSON() ?? null,
			falseType: this.falseType?.toJSON() ?? null
		};
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "TypeConditional");
	}
};
/** `TypeObjectField` — key + right + flags. */
var RemoteTypeObjectField = class {
	type = "TypeObjectField";
	_internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this._internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile
		};
	}
	get range() {
		return absoluteRange(this._internal);
	}
	get parent() {
		return this._internal.parent;
	}
	get optional() {
		return (commonData(this._internal) & 1) !== 0;
	}
	get readonly() {
		return (commonData(this._internal) & 2) !== 0;
	}
	get quote() {
		return commonData(this._internal) >> 2 & 3;
	}
	get key() {
		return childNodeAtVisitorIndexChildren(this._internal, 0);
	}
	get right() {
		return childNodeAtVisitorIndexChildren(this._internal, 1);
	}
	toJSON() {
		return {
			type: this.type,
			range: [...this.range],
			optional: this.optional,
			readonly: this.readonly,
			quote: this.quote,
			key: this.key?.toJSON() ?? null,
			right: this.right?.toJSON() ?? null
		};
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "TypeObjectField");
	}
};
/** `TypeJsdocObjectField` — key + right (no flags). */
var RemoteTypeJsdocObjectField = class {
	type = "TypeJsdocObjectField";
	_internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this._internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile
		};
	}
	get range() {
		return absoluteRange(this._internal);
	}
	get parent() {
		return this._internal.parent;
	}
	get key() {
		return childNodeAtVisitorIndexChildren(this._internal, 0);
	}
	get right() {
		return childNodeAtVisitorIndexChildren(this._internal, 1);
	}
	toJSON() {
		return {
			type: this.type,
			range: [...this.range],
			key: this.key?.toJSON() ?? null,
			right: this.right?.toJSON() ?? null
		};
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "TypeJsdocObjectField");
	}
};
/** Signature container (CallSignature / ConstructorSignature). */
function defineSignature(typeName) {
	return class {
		type = typeName;
		_internal;
		constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
			this._internal = {
				view,
				byteIndex,
				index,
				rootIndex,
				parent,
				sourceFile
			};
		}
		get range() {
			return absoluteRange(this._internal);
		}
		get parent() {
			return this._internal.parent;
		}
		get parameters() {
			return childNodeAtVisitorIndexChildren(this._internal, 0);
		}
		get returnType() {
			return childNodeAtVisitorIndexChildren(this._internal, 1);
		}
		get typeParameters() {
			return childNodeAtVisitorIndexChildren(this._internal, 2);
		}
		toJSON() {
			return {
				type: this.type,
				range: [...this.range],
				parameters: this.parameters?.toJSON() ?? null,
				returnType: this.returnType?.toJSON() ?? null,
				typeParameters: this.typeParameters?.toJSON() ?? null
			};
		}
		[inspectSymbol]() {
			return inspectPayload(this.toJSON(), typeName);
		}
	};
}
var RemoteTypeCallSignature = class extends defineSignature("TypeCallSignature") {};
var RemoteTypeConstructorSignature = class extends defineSignature("TypeConstructorSignature") {};
/** `TypeKeyValue` — key string in Extended Data + first child as `right`. */
var RemoteTypeKeyValue = class {
	type = "TypeKeyValue";
	_internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this._internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile
		};
	}
	get range() {
		return absoluteRange(this._internal);
	}
	get parent() {
		return this._internal.parent;
	}
	get optional() {
		return (commonData(this._internal) & 1) !== 0;
	}
	get variadic() {
		return (commonData(this._internal) & 2) !== 0;
	}
	get key() {
		return extStringLeaf(this._internal);
	}
	get right() {
		const head = firstChildIndex(this._internal.sourceFile, this._internal.index);
		if (head === 0) return null;
		return this._internal.sourceFile.getNode(head, thisNode(this._internal), this._internal.rootIndex);
	}
	toJSON() {
		return {
			type: this.type,
			range: [...this.range],
			optional: this.optional,
			variadic: this.variadic,
			key: this.key,
			right: this.right?.toJSON() ?? null
		};
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "TypeKeyValue");
	}
};
/** Helper for `TypeIndexSignature` / `TypeMappedType` — key + first child. */
function defineKeyAndChild(typeName) {
	return class {
		type = typeName;
		_internal;
		constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
			this._internal = {
				view,
				byteIndex,
				index,
				rootIndex,
				parent,
				sourceFile
			};
		}
		get range() {
			return absoluteRange(this._internal);
		}
		get parent() {
			return this._internal.parent;
		}
		get key() {
			return extStringLeaf(this._internal);
		}
		get right() {
			const head = firstChildIndex(this._internal.sourceFile, this._internal.index);
			if (head === 0) return null;
			return this._internal.sourceFile.getNode(head, thisNode(this._internal), this._internal.rootIndex);
		}
		toJSON() {
			return {
				type: this.type,
				range: [...this.range],
				key: this.key,
				right: this.right?.toJSON() ?? null
			};
		}
		[inspectSymbol]() {
			return inspectPayload(this.toJSON(), typeName);
		}
	};
}
var RemoteTypeIndexSignature = class extends defineKeyAndChild("TypeIndexSignature") {};
var RemoteTypeMappedType = class extends defineKeyAndChild("TypeMappedType") {};
/** `TypeMethodSignature` — name string in Extended Data + Common Data flags. */
var RemoteTypeMethodSignature = class {
	type = "TypeMethodSignature";
	_internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this._internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile
		};
	}
	get range() {
		return absoluteRange(this._internal);
	}
	get parent() {
		return this._internal.parent;
	}
	get quote() {
		return commonData(this._internal) & 3;
	}
	get hasParameters() {
		return (commonData(this._internal) & 4) !== 0;
	}
	get hasTypeParameters() {
		return (commonData(this._internal) & 8) !== 0;
	}
	get name() {
		return extStringLeaf(this._internal);
	}
	toJSON() {
		return {
			type: this.type,
			range: [...this.range],
			quote: this.quote,
			hasParameters: this.hasParameters,
			hasTypeParameters: this.hasTypeParameters,
			name: this.name
		};
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "TypeMethodSignature");
	}
};
/** `TypeTemplateLiteral` — literal-segment array in Extended Data. */
var RemoteTypeTemplateLiteral = class {
	type = "TypeTemplateLiteral";
	_internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this._internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile
		};
	}
	get range() {
		return absoluteRange(this._internal);
	}
	get parent() {
		return this._internal.parent;
	}
	/** Number of literal segments stored at byte 0-1 of Extended Data. */
	get literalCount() {
		return this._internal.view.getUint16(extOffsetOf(this._internal), true);
	}
	/** Resolve the n-th literal segment. */
	literal(index) {
		const off = extOffsetOf(this._internal) + 2 + index * 6;
		const offset = this._internal.view.getUint32(off, true);
		const length = this._internal.view.getUint16(off + 4, true);
		return this._internal.sourceFile.getStringByField(offset, length) ?? "";
	}
	/** All literal segments as an array. */
	get literals() {
		const count = this.literalCount;
		const out = Array.from({ length: count });
		for (let i = 0; i < count; i++) out[i] = this.literal(i);
		return out;
	}
	toJSON() {
		return {
			type: this.type,
			range: [...this.range],
			literals: this.literals
		};
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "TypeTemplateLiteral");
	}
};
/** `TypeSymbol` — `Symbol(...)` callee value + optional element. */
var RemoteTypeSymbol = class {
	type = "TypeSymbol";
	_internal;
	constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
		this._internal = {
			view,
			byteIndex,
			index,
			rootIndex,
			parent,
			sourceFile
		};
	}
	get range() {
		return absoluteRange(this._internal);
	}
	get parent() {
		return this._internal.parent;
	}
	get hasElement() {
		return (commonData(this._internal) & 1) !== 0;
	}
	get value() {
		return extStringLeaf(this._internal);
	}
	get element() {
		if (!this.hasElement) return null;
		const head = firstChildIndex(this._internal.sourceFile, this._internal.index);
		if (head === 0) return null;
		return this._internal.sourceFile.getNode(head, thisNode(this._internal), this._internal.rootIndex);
	}
	toJSON() {
		return {
			type: this.type,
			range: [...this.range],
			hasElement: this.hasElement,
			value: this.value,
			element: this.element?.toJSON() ?? null
		};
	}
	[inspectSymbol]() {
		return inspectPayload(this.toJSON(), "TypeSymbol");
	}
};
var RemoteTypeNull = class extends definePureLeaf("TypeNull") {};
var RemoteTypeUndefined = class extends definePureLeaf("TypeUndefined") {};
var RemoteTypeAny = class extends definePureLeaf("TypeAny") {};
var RemoteTypeUnknown = class extends definePureLeaf("TypeUnknown") {};
var RemoteTypeUniqueSymbol = class extends definePureLeaf("TypeUniqueSymbol") {};
//#endregion
//#region src/internal/kind-dispatch.ts
/**
* Kind → class dispatch table.
*
* Mirrors the Rust `Kind::from_u8` mapping. Phase 4 will code-generate
* this file from a single schema. Until then, every Kind discriminant
* (0x01-0x0F, 0x7F, 0x80-0xAC) is wired by hand.
*
* @author kazuya kawaguchi (a.k.a. kazupon)
* @license MIT
*/
/**
* Flat 256-entry table indexed by the Kind byte. `undefined` entries fall
* into the reserved space and trip an explicit error in `decodeKindToClass`.
*/
const KIND_TABLE = Array.from({ length: 256 });
KIND_TABLE[1] = RemoteJsdocBlock;
KIND_TABLE[2] = RemoteJsdocDescriptionLine;
KIND_TABLE[3] = RemoteJsdocTag;
KIND_TABLE[4] = RemoteJsdocTagName;
KIND_TABLE[5] = RemoteJsdocTagNameValue;
KIND_TABLE[6] = RemoteJsdocTypeSource;
KIND_TABLE[7] = RemoteJsdocTypeLine;
KIND_TABLE[8] = RemoteJsdocInlineTag;
KIND_TABLE[9] = RemoteJsdocGenericTagBody;
KIND_TABLE[10] = RemoteJsdocBorrowsTagBody;
KIND_TABLE[11] = RemoteJsdocRawTagBody;
KIND_TABLE[12] = RemoteJsdocParameterName;
KIND_TABLE[13] = RemoteJsdocNamepathSource;
KIND_TABLE[14] = RemoteJsdocIdentifier;
KIND_TABLE[15] = RemoteJsdocText;
KIND_TABLE[127] = RemoteNodeListNode;
KIND_TABLE[128] = RemoteTypeName;
KIND_TABLE[129] = RemoteTypeNumber;
KIND_TABLE[130] = RemoteTypeStringValue;
KIND_TABLE[131] = RemoteTypeNull;
KIND_TABLE[132] = RemoteTypeUndefined;
KIND_TABLE[133] = RemoteTypeAny;
KIND_TABLE[134] = RemoteTypeUnknown;
KIND_TABLE[135] = RemoteTypeUnion;
KIND_TABLE[136] = RemoteTypeIntersection;
KIND_TABLE[137] = RemoteTypeGeneric;
KIND_TABLE[138] = RemoteTypeFunction;
KIND_TABLE[139] = RemoteTypeObject;
KIND_TABLE[140] = RemoteTypeTuple;
KIND_TABLE[141] = RemoteTypeParenthesis;
KIND_TABLE[142] = RemoteTypeNamePath;
KIND_TABLE[143] = RemoteTypeSpecialNamePath;
KIND_TABLE[144] = RemoteTypeNullable;
KIND_TABLE[145] = RemoteTypeNotNullable;
KIND_TABLE[146] = RemoteTypeOptional;
KIND_TABLE[147] = RemoteTypeVariadic;
KIND_TABLE[148] = RemoteTypeConditional;
KIND_TABLE[149] = RemoteTypeInfer;
KIND_TABLE[150] = RemoteTypeKeyOf;
KIND_TABLE[151] = RemoteTypeTypeOf;
KIND_TABLE[152] = RemoteTypeImport;
KIND_TABLE[153] = RemoteTypePredicate;
KIND_TABLE[154] = RemoteTypeAsserts;
KIND_TABLE[155] = RemoteTypeAssertsPlain;
KIND_TABLE[156] = RemoteTypeReadonlyArray;
KIND_TABLE[157] = RemoteTypeTemplateLiteral;
KIND_TABLE[158] = RemoteTypeUniqueSymbol;
KIND_TABLE[159] = RemoteTypeSymbol;
KIND_TABLE[160] = RemoteTypeObjectField;
KIND_TABLE[161] = RemoteTypeJsdocObjectField;
KIND_TABLE[162] = RemoteTypeKeyValue;
KIND_TABLE[163] = RemoteTypeProperty;
KIND_TABLE[164] = RemoteTypeIndexSignature;
KIND_TABLE[165] = RemoteTypeMappedType;
KIND_TABLE[166] = RemoteTypeTypeParameter;
KIND_TABLE[167] = RemoteTypeCallSignature;
KIND_TABLE[168] = RemoteTypeConstructorSignature;
KIND_TABLE[169] = RemoteTypeMethodSignature;
KIND_TABLE[170] = RemoteTypeIndexedAccessIndex;
KIND_TABLE[171] = RemoteTypeParameterList;
KIND_TABLE[172] = RemoteTypeReadonlyProperty;
/**
* Look up the lazy class for a given Kind byte.
*/
function decodeKindToClass(kind) {
	const Class = KIND_TABLE[kind];
	if (Class === void 0) throw new Error(`unknown Kind: 0x${kind.toString(16).padStart(2, "0")}`);
	return Class;
}
//#endregion
//#region src/internal/source-file.ts
/**
* `RemoteSourceFile` — root of the JS lazy decoder.
*
* Mirrors the Rust `LazySourceFile`: parses the 40-byte Header at
* construction so every Remote* instance can reach the String table /
* Root array / Nodes section in O(1).
*
* Per js-decoder.md, all per-instance state lives in a single `#internal`
* object (V8 hidden-class friendly), and this is the only class that
* actually allocates caches (stringCache, nodeCache).
*
* @author kazuya kawaguchi (a.k.a. kazupon)
* @license MIT
*/
const utf8Decoder = new TextDecoder("utf-8");
/**
* Root of the lazy decoder. Construct one per Binary AST buffer.
*
* Public surface (used by Remote* node classes):
* - `view`, `extendedDataOffset`, `nodesOffset`, `nodeCount`, `rootCount`,
*   `compatMode` getters
* - `getString(idx)` — String Offsets[idx] → resolved string (cached)
* - `getRootBaseOffset(rootIndex)`
* - `getNode(nodeIndex, parent, rootIndex)` — lazy class instance (cached)
* - `asts` getter — array of root Remote* instances (or `null` for failures)
*/
var RemoteSourceFile = class {
	#internal;
	/**
	* Construct from a binary buffer (any `BufferSource`-compatible value).
	*
	* Throws when:
	* - the buffer is shorter than {@link HEADER_SIZE} bytes
	* - the major version disagrees with {@link SUPPORTED_MAJOR}
	*
	* `emptyStringForNull`: only effective when the buffer's `compat_mode`
	* flag is set. Switches `toJSON()` and compat-mode field accessors to
	* emit `""` instead of `null` for absent optional strings (rawType,
	* name, namepathOrURL, text). Mirrors the Rust serializer's
	* `SerializeOptions.empty_string_for_null` for jsdoccomment parity.
	*/
	constructor(buffer, options) {
		const view = buffer instanceof ArrayBuffer ? new DataView(buffer) : new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);
		if (view.byteLength < 40) throw new Error(`buffer too short: ${view.byteLength} bytes (need at least 40)`);
		const versionByte = view.getUint8(0);
		const major = versionByte >>> 4;
		if (major !== 1) throw new Error(`incompatible Binary AST major version: buffer=${major}, decoder=1`);
		if ((view.byteOffset & 3) !== 0) throw new Error(`Binary AST buffer must be 4-byte aligned (byteOffset=${view.byteOffset})`);
		const uint32View = new Uint32Array(view.buffer, view.byteOffset, view.byteLength >>> 2);
		const flags = view.getUint8(1);
		const nodeCount = uint32View[7];
		const compatMode = (flags & 1) !== 0;
		this.#internal = {
			view,
			uint32View,
			version: versionByte,
			compatMode,
			emptyStringForNull: compatMode && options !== void 0 && options.emptyStringForNull === true,
			rootArrayOffset: uint32View[1],
			stringOffsetsOffset: uint32View[2],
			stringDataOffset: uint32View[3],
			extendedDataOffset: uint32View[4],
			diagnosticsOffset: uint32View[5],
			nodesOffset: uint32View[6],
			nodeCount,
			sourceTextLength: uint32View[8],
			rootCount: uint32View[9],
			stringCache: /* @__PURE__ */ new Map(),
			nodeCache: new Array(nodeCount),
			$asts: void 0
		};
	}
	/** Underlying DataView. */
	get view() {
		return this.#internal.view;
	}
	/**
	* Underlying typed `Uint32Array` view aligned to the buffer start.
	* Index by `byteOffset >>> 2` for any 4-byte aligned u32 read; this is
	* 5–10× faster than `DataView.getUint32` in V8 hot paths.
	*/
	get uint32View() {
		return this.#internal.uint32View;
	}
	/** Whether the buffer's `compat_mode` flag bit is set. */
	get compatMode() {
		return this.#internal.compatMode;
	}
	/** Whether `null` optional strings are emitted as `""` in compat-mode. */
	get emptyStringForNull() {
		return this.#internal.emptyStringForNull;
	}
	/** Byte offset of the Extended Data section. */
	get extendedDataOffset() {
		return this.#internal.extendedDataOffset;
	}
	/** Byte offset of the Nodes section. */
	get nodesOffset() {
		return this.#internal.nodesOffset;
	}
	/** Total number of node records (including the `node[0]` sentinel). */
	get nodeCount() {
		return this.#internal.nodeCount;
	}
	/** Number of roots N. */
	get rootCount() {
		return this.#internal.rootCount;
	}
	/**
	* Resolve the string at `idx` (returns `null` for the
	* `STRING_PAYLOAD_NONE_SENTINEL` (`0x3FFF_FFFF`) sentinel). Used by
	* string-leaf nodes (TypeTag::String payload) and the diagnostics
	* section.
	*
	* Cached on first lookup so repeated reads are O(1).
	*/
	getString(idx) {
		if (idx === 1073741823) return null;
		const cached = this.#internal.stringCache.get(idx);
		if (cached !== void 0) return cached;
		const { view, uint32View, stringOffsetsOffset, stringDataOffset } = this.#internal;
		const entryWordIndex = stringOffsetsOffset + idx * 8 >>> 2;
		const start = uint32View[entryWordIndex];
		const end = uint32View[entryWordIndex + 1];
		const bytes = new Uint8Array(view.buffer, view.byteOffset + stringDataOffset + start, end - start);
		const str = utf8Decoder.decode(bytes);
		this.#internal.stringCache.set(idx, str);
		return str;
	}
	/**
	* Resolve a `StringField` `(offset, length)` pair into the underlying
	* string. Returns `null` when the field is the `NONE` sentinel
	* (`offset === STRING_FIELD_NONE_OFFSET`). Used by Extended Data string
	* slots which embed `(offset, length)` directly.
	*
	* Cache key uses a high-bit-set form of `offset` so it never collides
	* with `getString(idx)` cache entries (string-leaf path uses small
	* indices, ED path uses byte offsets — both fit in u32 and overlap).
	*/
	getStringByField(offset, length) {
		if (offset === 4294967295) return null;
		const cacheKey = -(offset + 1);
		const cached = this.#internal.stringCache.get(cacheKey);
		if (cached !== void 0) return cached;
		const { view, stringDataOffset } = this.#internal;
		const bytes = new Uint8Array(view.buffer, view.byteOffset + stringDataOffset + offset, length);
		const str = utf8Decoder.decode(bytes);
		this.#internal.stringCache.set(cacheKey, str);
		return str;
	}
	/**
	* Resolve a Path B-leaf inline `(offset, length)` pair into the underlying
	* string. Always returns a real `&str` (never `null`) — encoders only
	* emit `TypeTag::StringInline` for present, non-empty short strings.
	*
	* Reuses the same cache-key disambiguation as `getStringByField` (offset
	* is tagged with the sign bit) so inline-path lookups never collide with
	* String-Offsets-table lookups.
	*/
	getStringByOffsetAndLength(offset, length) {
		const cacheKey = -(offset + 1);
		const cached = this.#internal.stringCache.get(cacheKey);
		if (cached !== void 0) return cached;
		const { view, stringDataOffset } = this.#internal;
		const bytes = new Uint8Array(view.buffer, view.byteOffset + stringDataOffset + offset, length);
		const str = utf8Decoder.decode(bytes);
		this.#internal.stringCache.set(cacheKey, str);
		return str;
	}
	/**
	* Get the `base_offset` for the i-th root (used to compute absolute ranges).
	*/
	getRootBaseOffset(rootIndex) {
		const off = this.#internal.rootArrayOffset + rootIndex * 12 + 8;
		return this.#internal.uint32View[off >>> 2];
	}
	/**
	* Get the `source_offset_in_data` (byte offset where this root's source
	* text starts inside the String Data section) for the i-th root.
	* Used by `descriptionRaw` getters that need to slice the source text
	* by `(start, end)` byte offsets.
	*/
	getRootSourceOffsetInData(rootIndex) {
		const off = this.#internal.rootArrayOffset + rootIndex * 12 + 4;
		return this.#internal.uint32View[off >>> 2];
	}
	/**
	* Slice the source text region for `rootIndex` at the given
	* `(start, end)` source-text-relative UTF-8 byte offsets. Returns
	* `null` for the `(0, 0)` sentinel, for `start > end`, or when the
	* slice would extend past the buffer.
	*
	* Used by `descriptionRaw` getters on `RemoteJsdocBlock` /
	* `RemoteJsdocTag` (compat-mode wire field per
	* `design/008-oxlint-oxfmt-support/README.md` §4.3).
	*/
	sliceSourceText(rootIndex, start, end) {
		if (start === 0 && end === 0) return null;
		if (start > end) return null;
		const sourceOffset = this.getRootSourceOffsetInData(rootIndex);
		const { view, stringDataOffset } = this.#internal;
		const absStart = stringDataOffset + sourceOffset + start;
		if (stringDataOffset + sourceOffset + end > view.byteOffset + view.byteLength) return null;
		const bytes = new Uint8Array(view.buffer, view.byteOffset + absStart, end - start);
		return utf8Decoder.decode(bytes);
	}
	/**
	* Return the complete source text for one root.
	*/
	getRootSourceText(rootIndex) {
		const sourceOffset = this.getRootSourceOffsetInData(rootIndex);
		const nextOffset = rootIndex + 1 < this.#internal.rootCount ? this.getRootSourceOffsetInData(rootIndex + 1) : this.#internal.sourceTextLength;
		if (nextOffset < sourceOffset) return "";
		const { view, stringDataOffset } = this.#internal;
		const bytes = new Uint8Array(view.buffer, view.byteOffset + stringDataOffset + sourceOffset, nextOffset - sourceOffset);
		return utf8Decoder.decode(bytes);
	}
	/**
	* Build (or fetch from cache) the lazy class instance for a node.
	*
	* Returns `null` for the sentinel (node index 0).
	*/
	getNode(nodeIndex, parent, rootIndex = -1) {
		if (nodeIndex === 0) return null;
		const cached = this.#internal.nodeCache[nodeIndex];
		if (cached !== void 0) return cached;
		const byteIndex = this.#internal.nodesOffset + nodeIndex * 24;
		const node = new (decodeKindToClass(this.#internal.view.getUint8(byteIndex + 0)))(this.#internal.view, byteIndex, nodeIndex, rootIndex, parent, this);
		this.#internal.nodeCache[nodeIndex] = node;
		return node;
	}
	/**
	* AST root for each entry in the Root Index array. Yields `null` for
	* entries with `node_index === 0` (parse failure sentinel) and the
	* matching lazy class instance otherwise.
	*/
	get asts() {
		if (this.#internal.$asts !== void 0) return this.#internal.$asts;
		const { view, rootArrayOffset, rootCount } = this.#internal;
		const result = new Array(rootCount);
		for (let i = 0; i < rootCount; i++) {
			const nodeIdx = view.getUint32(rootArrayOffset + i * 12 + 0, true);
			result[i] = nodeIdx === 0 ? null : this.getNode(nodeIdx, null, i);
		}
		this.#internal.$asts = result;
		return result;
	}
};
//#endregion
//#region src/index.ts
/**
* Visitor keys for every Remote* node kind (60 = 15 Comment AST + 45 TypeNode).
*
* Each entry maps a node `type` name to the **traversable child property
* names** in canonical visit order. Mirrors the jsdoccomment / ESLint
* `visitorKeys` convention — frameworks that depend on it (`estraverse`,
* `eslint-visitor-keys`, etc.) can spread this object directly into their
* own key map.
*
* **Differences from jsdoccomment**: ox-jsdoc emits `JsdocTag.tag` /
* `rawType` / `name` / `body` as actual child nodes (`RemoteJsdocTagName`,
* `RemoteJsdocTypeSource`, `RemoteJsdocTagNameValue`, `RemoteJsdocTagBody`)
* instead of flattening them to strings, so those property names appear in
* `JsdocTag`'s key list. Strict-jsdoccomment consumers can filter the list
* down to `['parsedType', 'typeLines', 'descriptionLines', 'inlineTags']`.
*
* **Reserved kinds** (`JsdocBorrowsTagBody`, `JsdocRawTagBody`) are listed
* for future use; the parser does not currently emit them
* (see `design/007-binary-ast/ast-nodes.md` "Reserved Kinds").
*/
const jsdocVisitorKeys = Object.freeze({
	JsdocBlock: [
		"descriptionLines",
		"tags",
		"inlineTags"
	],
	JsdocDescriptionLine: [],
	JsdocTag: [
		"tag",
		"rawType",
		"name",
		"parsedType",
		"body",
		"typeLines",
		"descriptionLines",
		"inlineTags"
	],
	JsdocTagName: [],
	JsdocTagNameValue: [],
	JsdocTypeSource: [],
	JsdocTypeLine: [],
	JsdocInlineTag: [],
	JsdocGenericTagBody: ["typeSource", "value"],
	JsdocBorrowsTagBody: ["source", "target"],
	JsdocRawTagBody: [],
	JsdocParameterName: [],
	JsdocNamepathSource: [],
	JsdocIdentifier: [],
	JsdocText: [],
	TypeName: [],
	TypeNumber: [],
	TypeStringValue: [],
	TypeProperty: [],
	TypeSpecialNamePath: [],
	TypeNull: [],
	TypeUndefined: [],
	TypeAny: [],
	TypeUnknown: [],
	TypeUniqueSymbol: [],
	TypeUnion: ["elements"],
	TypeIntersection: ["elements"],
	TypeObject: ["elements"],
	TypeTuple: ["elements"],
	TypeTypeParameter: ["elements"],
	TypeParameterList: ["elements"],
	TypeParenthesis: ["element"],
	TypeInfer: ["element"],
	TypeKeyOf: ["element"],
	TypeTypeOf: ["element"],
	TypeImport: ["element"],
	TypeAssertsPlain: ["element"],
	TypeReadonlyArray: ["element"],
	TypeIndexedAccessIndex: ["element"],
	TypeReadonlyProperty: ["element"],
	TypeNullable: ["element"],
	TypeNotNullable: ["element"],
	TypeOptional: ["element"],
	TypeVariadic: ["element"],
	TypePredicate: ["left", "right"],
	TypeAsserts: ["left", "right"],
	TypeNamePath: ["left", "right"],
	TypeGeneric: ["left", "elements"],
	TypeFunction: [
		"parameters",
		"returnType",
		"typeParameters"
	],
	TypeConditional: [
		"checkType",
		"extendsType",
		"trueType",
		"falseType"
	],
	TypeObjectField: ["key", "right"],
	TypeJsdocObjectField: ["key", "right"],
	TypeKeyValue: ["right"],
	TypeIndexSignature: ["right"],
	TypeMappedType: ["right"],
	TypeMethodSignature: [
		"parameters",
		"returnType",
		"typeParameters"
	],
	TypeCallSignature: [
		"parameters",
		"returnType",
		"typeParameters"
	],
	TypeConstructorSignature: [
		"parameters",
		"returnType",
		"typeParameters"
	],
	TypeTemplateLiteral: [],
	TypeSymbol: ["element"]
});
/**
* Recursively convert a Remote* lazy node into a plain JSON object.
* Handy for browser DevTools (where `Symbol.for('nodejs.util.inspect.custom')`
* has no effect) and for general logging.
*/
function toPlainObject(node) {
	if (node === null || node === void 0) return node;
	if (typeof node !== "object") return node;
	if (Array.isArray(node)) return node.map(toPlainObject);
	const candidate = node;
	if (typeof candidate.toJSON === "function") return candidate.toJSON();
	return node;
}
//#endregion
export { EMPTY_NODE_LIST, RemoteJsdocBlock, RemoteJsdocBorrowsTagBody, RemoteJsdocDescriptionLine, RemoteJsdocGenericTagBody, RemoteJsdocIdentifier, RemoteJsdocInlineTag, RemoteJsdocNamepathSource, RemoteJsdocParameterName, RemoteJsdocRawTagBody, RemoteJsdocTag, RemoteJsdocTagName, RemoteJsdocTagNameValue, RemoteJsdocText, RemoteJsdocTypeLine, RemoteJsdocTypeSource, RemoteNodeList, RemoteNodeListNode, RemoteSourceFile, RemoteTypeAny, RemoteTypeAsserts, RemoteTypeAssertsPlain, RemoteTypeCallSignature, RemoteTypeConditional, RemoteTypeConstructorSignature, RemoteTypeFunction, RemoteTypeGeneric, RemoteTypeImport, RemoteTypeIndexSignature, RemoteTypeIndexedAccessIndex, RemoteTypeInfer, RemoteTypeIntersection, RemoteTypeJsdocObjectField, RemoteTypeKeyOf, RemoteTypeKeyValue, RemoteTypeMappedType, RemoteTypeMethodSignature, RemoteTypeName, RemoteTypeNamePath, RemoteTypeNotNullable, RemoteTypeNull, RemoteTypeNullable, RemoteTypeNumber, RemoteTypeObject, RemoteTypeObjectField, RemoteTypeOptional, RemoteTypeParameterList, RemoteTypeParenthesis, RemoteTypePredicate, RemoteTypeProperty, RemoteTypeReadonlyArray, RemoteTypeReadonlyProperty, RemoteTypeSpecialNamePath, RemoteTypeStringValue, RemoteTypeSymbol, RemoteTypeTemplateLiteral, RemoteTypeTuple, RemoteTypeTypeOf, RemoteTypeTypeParameter, RemoteTypeUndefined, RemoteTypeUnion, RemoteTypeUniqueSymbol, RemoteTypeUnknown, RemoteTypeVariadic, jsdocVisitorKeys, toPlainObject };
