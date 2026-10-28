import {
  parseComment,
  parseCommentBatch,
} from '@ox-jsdoc/jsdoccomment';

/**
 * @typedef {'single' | 'batch'} OxParseStrategy
 */

/**
 * @typedef {import('estree').Comment | import('eslint').AST.Token | {
 *   value: string,
 *   range?: [number, number]
 * }} CommentNode
 */

/**
 * @typedef {{
 *   oxParseStrategy?: OxParseStrategy
 * }} ParseSettings
 */

/**
 * @type {WeakMap<
 *   import('eslint').SourceCode,
 *   Map<string, WeakMap<CommentNode, import('@ox-jsdoc/jsdoccomment').JsdocBlockWithInline>>
 * >}
 */
const batchCacheBySourceCode = new WeakMap();

/**
 * @param {ParseSettings} settings
 * @returns {OxParseStrategy}
 */
const getOxParseStrategy = (settings) => {
  return settings.oxParseStrategy === 'batch' ? 'batch' : 'single';
};

/**
 * @param {CommentNode} comment
 * @returns {boolean}
 */
const isJsdocComment = (comment) => {
  return (/^\*(?!\*)/v).test(comment.value);
};

/**
 * @param {import('eslint').SourceCode} sourceCode
 * @param {string} indent
 * @returns {WeakMap<CommentNode, import('@ox-jsdoc/jsdoccomment').JsdocBlockWithInline>}
 */
const getBatchCache = (sourceCode, indent) => {
  let cacheByIndent = batchCacheBySourceCode.get(sourceCode);
  if (!cacheByIndent) {
    cacheByIndent = new Map();
    batchCacheBySourceCode.set(sourceCode, cacheByIndent);
  }

  const existing = cacheByIndent.get(indent);
  if (existing) {
    return existing;
  }

  const comments = /** @type {CommentNode[]} */ (
    sourceCode.getAllComments()
  ).filter(isJsdocComment);
  const {
    blocks,
  } = parseCommentBatch(comments, {
    indent,
  });
  const cache = new WeakMap();

  for (const [
    index,
    comment,
  ] of comments.entries()) {
    const block = blocks[index];
    if (block) {
      cache.set(comment, block);
    }
  }

  cacheByIndent.set(indent, cache);
  return cache;
};

/**
 * In the batch strategy, the cached block is copied for each call. Rules
 * change the block they get (for example, the fixer of `sort-tags` splices
 * `source`), and ESLint and Oxlint call fixers as soon as a rule reports.
 * Without the copy, one rule's changes would reach the next rule.
 * `structuredClone` keeps the lines that `source` and `tags[].source` share.
 * @param {import('eslint').SourceCode} sourceCode
 * @param {CommentNode} commentNode
 * @param {ParseSettings} settings
 * @param {string} [indent]
 * @returns {import('@ox-jsdoc/jsdoccomment').JsdocBlockWithInline}
 */
const parseCommentForSource = (sourceCode, commentNode, settings, indent = '') => {
  if (getOxParseStrategy(settings) !== 'batch') {
    return parseComment(commentNode, indent);
  }

  const cached = getBatchCache(sourceCode, indent).get(commentNode);
  return cached ? structuredClone(cached) : parseComment(commentNode, indent);
};

export {
  getOxParseStrategy,
  parseCommentForSource,
};
