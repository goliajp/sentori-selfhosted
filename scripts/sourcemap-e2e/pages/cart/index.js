// One of two files called `index.js`.
//
// A WeChat mini program names every page's entry `index.js`, and a web
// build names every route chunk after its route. So a basename is not
// an identifier, and a server matching on one picks a map that
// resolves to a plausible line in the wrong file — worse than not
// resolving, because nothing about the result says it is wrong.
//
// The two differ in where the throw sits, so a map picked by basename
// alone resolves to the other file's function and the test can tell.

function cartTotal() {
  throw new Error('cart boom')
}

export function openCart() {
  return cartTotal()
}
