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
});
