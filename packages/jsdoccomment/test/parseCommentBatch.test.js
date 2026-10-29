import {
  parse as commentParserParse
} from 'comment-parser';

import {
  parseComment,
  parseCommentBatch
} from '../src/index.js';

describe('parseCommentBatch', function () {
  it('Returns empty result for empty input', function () {
    expect(parseCommentBatch([])).to.deep.equal({
      blocks: [],
      problems: []
    });
  });

  it('Parses multiple comments with parseComment-compatible shape', function () {
    const comments = [
      '/** first */',
      '/**\n * @param {string} id\n */'
    ];
    const parsed = parseCommentBatch(comments);

    expect(parsed.blocks).to.deep.equal(comments.map(comment => {
      return parseComment(comment);
    }));
    expect(parsed.problems).to.deep.equal([]);
  });

  it('Accepts ESLint comment token-like input', function () {
    const comment = {value: '* @see SomeName'};
    const parsed = parseCommentBatch([comment]);

    expect(parsed.blocks[0]).to.deep.equal(parseComment(comment));
    expect(parsed.problems).to.deep.equal([]);
  });

  it('Passes indent through like parseComment', function () {
    const comment = {value: '* @template SomeName'};
    const parsed = parseCommentBatch([comment], {
      indent: '  '
    });

    expect(parsed.blocks[0]).to.deep.equal(parseComment(comment, '  '));
    expect(parsed.problems).to.deep.equal([]);
  });

  it('Ignores binary inline tag diagnostics for parsed blocks', function () {
    const parsed = parseCommentBatch(['/**\n * {@link foo\n */']);

    expect(parsed.blocks[0].problems).to.deep.equal([]);
    expect(parsed.problems).to.deep.equal([]);
  });

  it('Keeps failed items as null with indexed problems', function () {
    const parsed = parseCommentBatch([
      '/** ok */',
      '/* not jsdoc */'
    ]);

    expect(parsed.blocks[0]).to.deep.equal(parseComment('/** ok */'));
    expect(parsed.blocks[1]).to.equal(null);
    expect(parsed.problems).to.have.length(1);
    expect(parsed.problems[0].index).to.equal(1);
    expect(parsed.problems[0].problem.code).to.equal('custom');
    expect(parsed.problems[0].problem.message).to.contain('not a JSDoc block');
  });

  it('Throws on failed items when throwOnError is true', function () {
    expect(() => {
      parseCommentBatch([
        '/** ok */',
        '/* not jsdoc */'
      ], {
        throwOnError: true
      });
    }).to.throw('index 1');
  });

  it('Gives the same blocks as parseComment for tag names with punctuation', function () {
    // parseComment used to cut the tag name at the punctuation (`transient`).
    const comments = [
      '/**\n * @transient()\n */',
      '/**\n * @foo.bar baz\n */',
      '/**\n * @foo:bar baz\n */'
    ];
    const parsed = parseCommentBatch(comments);

    expect(parsed.blocks).to.deep.equal(comments.map(comment => {
      return parseComment(comment);
    }));
    expect(comments.map(comment => {
      return parseComment(comment).tags[0].tag;
    })).to.deep.equal(comments.map(comment => {
      return commentParserParse(comment)[0].tags[0].tag;
    }));
  });

  it('Gives the same blocks as parseComment after non-ASCII text', function () {
    const comments = [
      '/**\n * 日本語の説明です。\n * @param a 引数\n * @returns 戻り値\n */',
      '/**\n * 一行目の説明。\n * 二行目の説明。\n * @param a 引数\n * @throws {Error} 例外\n * @returns 戻り値\n */',
      '/**\n * Party 🎉 time.\n * @param a arg\n * @returns value\n */'
    ];
    const parsed = parseCommentBatch(comments);

    expect(parsed.blocks).to.deep.equal(comments.map(comment => {
      return parseComment(comment);
    }));
  });
});
