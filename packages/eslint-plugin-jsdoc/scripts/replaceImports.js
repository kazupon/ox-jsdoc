import replace from 'replace';

// Cross-platform replacement for the two `replace` CLI calls in `build`:
//   replace 'to-valid-identifier' '../to-valid-identifier.cjs' 'dist' -r --include="*.cjs"
//   replace 'require\("\.(.*?)\.[^.]*?"\)' 'require(".$1.cjs")' 'dist' -r --include="*.cjs"
// The CLI form fails on Windows because cmd.exe does not parse single-quoted
// arguments, so the regex / replacement strings are passed through here via
// `replace`'s programmatic API instead.

const sharedOptions = {
  paths: ['dist'],
  recursive: true,
  include: '*.cjs',
  silent: true,
};

replace({
  ...sharedOptions,
  regex: 'to-valid-identifier',
  replacement: '../to-valid-identifier.cjs',
});

replace({
  ...sharedOptions,
  regex: 'require\\("\\.(.*?)\\.[^.]*?"\\)',
  replacement: 'require(".$1.cjs")',
});
