// Phase 16 sub-E source-map e2e fixture.
// Deliberately deep enough that a minified bundle has interesting line
// numbers to symbolicate.

function level3(): never {
  throw new Error('sourcemap e2e — level3 boom')
}

function level2() {
  return level3()
}

function level1() {
  return level2()
}

level1()

export {}
