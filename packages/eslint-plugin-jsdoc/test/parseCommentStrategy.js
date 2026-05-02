import {
  getOxParseStrategy,
  parseCommentForSource,
} from '../src/parseCommentStrategy.js';
import {
  expect,
} from 'chai';

describe('parseCommentStrategy', () => {
  describe('getOxParseStrategy', () => {
    it('defaults to single-comment parsing', () => {
      expect(getOxParseStrategy({})).to.equal('single');
      expect(getOxParseStrategy({
        // @ts-expect-error Deliberately invalid setting.
        oxParseStrategy: 'other',
      })).to.equal('single');
    });

    it('accepts batch parsing explicitly', () => {
      expect(getOxParseStrategy({
        oxParseStrategy: 'batch',
      })).to.equal('batch');
    });
  });

  describe('parseCommentForSource', () => {
    it('parses a single comment without reading source comments by default', () => {
      const comment = {
        value: '* Description.\n * @param {number} count Count.\n ',
      };
      const sourceCode = /** @type {import('eslint').SourceCode} */ (
        /** @type {unknown} */ ({
          getAllComments () {
            throw new Error('single strategy should not build the batch cache');
          },
        })
      );

      const parsed = parseCommentForSource(sourceCode, comment, {}, '');

      expect(parsed.description).to.equal('Description.');
      expect(parsed.tags).to.have.length(1);
      expect(parsed.tags[0].name).to.equal('count');
      expect(parsed.tags[0].type).to.equal('number');
    });

    it('parses all JSDoc comments with the batch strategy and caches them', () => {
      const firstComment = {
        value: '* First.\n * @param {string} first First.\n ',
      };
      const secondComment = {
        value: '* Second.\n * @returns {number} Second.\n ',
      };
      const nonJsdocComment = {
        value: ' not a jsdoc block ',
      };
      let getAllCommentsCalls = 0;
      const sourceCode = /** @type {import('eslint').SourceCode} */ (
        /** @type {unknown} */ ({
          getAllComments () {
            getAllCommentsCalls++;
            return [
              firstComment,
              nonJsdocComment,
              secondComment,
            ];
          },
        })
      );
      const settings = /** @type {import('../src/parseCommentStrategy.js').ParseSettings} */ ({
        oxParseStrategy: 'batch',
      });

      const firstParsed = parseCommentForSource(
        sourceCode,
        firstComment,
        settings,
        '',
      );
      const secondParsed = parseCommentForSource(
        sourceCode,
        secondComment,
        settings,
        '',
      );

      expect(getAllCommentsCalls).to.equal(1);
      expect(firstParsed.description).to.equal('First.');
      expect(firstParsed.tags[0].name).to.equal('first');
      expect(secondParsed.description).to.equal('Second.');
      expect(secondParsed.tags[0].tag).to.equal('returns');
      expect(parseCommentForSource(
        sourceCode,
        firstComment,
        settings,
        '',
      )).to.equal(firstParsed);
      expect(getAllCommentsCalls).to.equal(1);
    });
  });
});
