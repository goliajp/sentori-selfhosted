// Source-map e2e fixture.
//
// Deep enough that the minified bundle has interesting line numbers to
// symbolicate. Plain JS rather than TSX: the only TypeScript in it was
// a `: never` annotation, and dropping it means the bundling step needs
// Metro alone instead of Metro plus a Babel preset — one fewer thing
// for the e2e to depend on.

function level3() {
  throw new Error('sourcemap e2e — level3 boom')
}

function level2() {
  return level3()
}

function level1() {
  return level2()
}

level1()
