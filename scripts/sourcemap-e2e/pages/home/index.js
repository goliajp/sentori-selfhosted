// The other `index.js`. Deliberately longer, so a map picked by
// basename resolves the cart's frame to a line that exists here and
// names a different function.

function homeBanner() {
  return 'banner'
}

function homeFeed() {
  return [homeBanner()]
}

function homeGreeting() {
  return 'hello'
}

export function openHome() {
  return [homeFeed(), homeGreeting()]
}
