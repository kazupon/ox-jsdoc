import { stringify } from 'comment-parser';

import { parseComment } from '../src/index.js';

describe('parseComment compatibility', function () {
  it('normalizes bracketed optional names and defaults', function () {
    const parsed = parseComment(
      '/**\n * @param {number} [code = 1] Exit code\n */'
    );
    const [tag] = parsed.tags;

    expect(tag.name).to.equal('code');
    expect(tag.type).to.equal('number');
    expect(tag.optional).to.equal(true);
    expect(tag.default).to.equal('1');
    expect(tag.description).to.equal('Exit code');
    expect(tag.source[0].tokens.name).to.equal('[code = 1]');
    expect(tag.source[0].tokens.description).to.equal('Exit code');
  });

  it('normalizes unbracketed defaults without changing the source token', function () {
    const parsed = parseComment(
      '/**\n * @property {number} BITMASK_VALUE_B=32 - the other thing\n */'
    );
    const [tag] = parsed.tags;

    expect(tag.name).to.equal('BITMASK_VALUE_B');
    expect(tag.optional).to.equal(false);
    expect(tag.default).to.equal('32');
    expect(tag.description).to.equal('- the other thing');
    expect(tag.source[0].tokens.name).to.equal('BITMASK_VALUE_B=32');
  });

  it('keeps default tags as descriptions instead of types', function () {
    const parsed = parseComment('/**\n * @default {}\n */');
    const [tag] = parsed.tags;

    expect(tag.type).to.equal('');
    expect(tag.description).to.equal('{}');
    expect(tag.source[0].tokens.type).to.equal('');
    expect(tag.source[0].tokens.description).to.equal('{}');
  });

  it('shares line objects between block source and tag source', function () {
    const parsed = parseComment('/**\n * @param {number} foo\n */');
    const [tag] = parsed.tags;

    expect(tag.source[0]).to.equal(parsed.source[1]);

    tag.source[0].tokens.type = '';
    tag.source[0].tokens.postTag = '';

    expect(stringify(parsed)).to.equal('/**\n * @param foo\n */');
  });

  it('reconstructs multiline typedef type and trailing name', function () {
    const parsed = parseComment(`/** Multi-line typedef.
 *
 * @typedef {{
 *   prop: number
 * }} MyOptions
 */`);
    const [tag] = parsed.tags;

    expect(tag.tag).to.equal('typedef');
    expect(tag.name).to.equal('MyOptions');
    expect(tag.type).to.equal('{\n  prop: number\n}');
    expect(tag.source[2].tokens.name).to.equal('MyOptions');
  });

  it('keeps continuation lines as tag description', function () {
    const parsed = parseComment(`/**
 * @param {string} lorem Description
 * with multiple lines.
 */`);
    const [tag] = parsed.tags;

    expect(parsed.description).to.equal('');
    expect(tag.description).to.equal('Description\nwith multiple lines.');
    expect(tag.source[1]).to.equal(parsed.source[2]);
  });

  it('preserves CRLF line endings in source tokens', function () {
    const parsed = parseComment(
      '/**\r\n * @param {string} lorem Description.\r\n */'
    );

    expect(parsed.source[0].source).to.equal('/**\r');
    expect(parsed.source[0].tokens.lineEnd).to.equal('\r');
    expect(parsed.tags[0].source[0].source).to.equal(
      ' * @param {string} lorem Description.\r'
    );
    expect(parsed.tags[0].source[0].tokens.lineEnd).to.equal('\r');
  });

  it('uses comment-parser critical problem shape for invalid names', function () {
    const parsed = parseComment('/**\n * @param [foo=] desc\n */');
    const [tag] = parsed.tags;
    const problem = {
      code: 'spec:name:empty-default',
      message: 'empty default value',
      line: 1,
      critical: true
    };

    expect(tag.name).to.equal('');
    expect(tag.type).to.equal('');
    expect(tag.optional).to.equal(false);
    expect(tag.description).to.equal('');
    expect(tag.source[0].tokens.name).to.equal('');
    expect(tag.source[0].tokens.description).to.equal('[foo=] desc');
    expect(tag.problems).to.deep.equal([problem]);
    expect(parsed.problems).to.deep.equal([problem]);
  });

  it('keeps fenced example contents out of the tag stream', function () {
    const parsed = parseComment(`/**
 * Registers.
 *
 * @param target - The class.
 *
 * @example \`\`\`ts
@transient()
class Foo { }
\`\`\`
 * @param Time for a new tag
 */`);

    expect(parsed.tags.map(tag => tag.tag)).to.deep.equal([
      'param',
      'example',
      'param'
    ]);
    expect(parsed.tags[1].description).to.equal(
      '```ts\n@transient()\nclass Foo { }\n```'
    );
    expect(parsed.tags[2].name).to.equal('Time');
    expect(parsed.tags[2].description).to.equal('for a new tag');
  });

  it('treats slash tag-looking text as block description', function () {
    const parsed = parseComment('/**\n * @ember/debug etc. etc.\n */');

    expect(parsed.description).to.equal('@ember/debug etc. etc.');
    expect(parsed.tags).to.deep.equal([]);
  });

  it('does not fold following tag lines into the previous tag', function () {
    const parsed = parseComment(`/**
 * Just a component.
 * @param {Object} props Свойства.
 * @return {ReactElement}.
 */`);
    const [paramTag, returnTag] = parsed.tags;

    expect(parsed.tags.map(tag => tag.tag)).to.deep.equal(['param', 'return']);
    expect(paramTag.description).to.equal('Свойства.');
    expect(returnTag.type).to.equal('ReactElement');
    expect(returnTag.description).to.equal('.');
    expect(paramTag.source.some(line => line.tokens.tag === '@return')).to.equal(
      false
    );
  });

  it('parses param names from continuation lines', function () {
    const parsed = parseComment(`/**
 * @param {string}
 *   foo The foo.
 */`);
    const [tag] = parsed.tags;

    expect(tag.name).to.equal('foo');
    expect(tag.description).to.equal('\n  The foo.');
    expect(tag.source[1].tokens.name).to.equal('foo');
    expect(tag.source[1].tokens.description).to.equal('The foo.');
  });

  it('does not treat comment openers inside descriptions as delimiters', function () {
    const parsed = parseComment(`/**
 * Description.
 * @param {string} b Description \`/**\`.
 */`);
    const [tag] = parsed.tags;

    expect(parsed.tags).to.have.length(1);
    expect(tag.tag).to.equal('param');
    expect(tag.name).to.equal('b');
    expect(tag.description).to.equal('Description `/**`.');
  });
});
