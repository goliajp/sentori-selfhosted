// Phase 21: ring buffer logic lives in @goliapkg/sentori-core. The
// public surface here keeps its object-form `addBreadcrumb({ type,
// data })` so existing callers don't break.
import { addBreadcrumb as addBreadcrumbCore, clearBreadcrumbs, getBreadcrumbs, } from '@goliapkg/sentori-core';
export function addBreadcrumb(input) {
    addBreadcrumbCore(input.type, input.data ?? {});
}
export { clearBreadcrumbs, getBreadcrumbs };
//# sourceMappingURL=breadcrumbs.js.map