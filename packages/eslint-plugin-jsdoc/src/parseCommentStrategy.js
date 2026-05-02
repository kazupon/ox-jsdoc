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

  const cache = getBatchCache(sourceCode, indent);
  return cache.get(commentNode) ?? parseComment(commentNode, indent);
};

export {
  getOxParseStrategy,
  parseCommentForSource,
};
