/** BrowserRouter commits history before React finishes its navigation render.
 * Merge control updates into that current URL so quick successive edits cannot
 * overwrite a condition with the previous render's query string. */
export function currentSearchParams(): URLSearchParams {
  return new URLSearchParams(window.location.search);
}
