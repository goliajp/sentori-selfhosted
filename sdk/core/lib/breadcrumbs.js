/**
 * Bounded ring buffer of breadcrumbs. Drops oldest first when the
 * cap is hit. Callers attach `breadcrumbs` to the next captured event;
 * we don't auto-flush — that's the SDK's job.
 *
 * The buffer is module-scoped so every `addBreadcrumb` call in the
 * host process writes to the same store; SDKs that need per-instance
 * state should construct their own `BreadcrumbBuffer` instead.
 */
const DEFAULT_CAP = 100;
export class BreadcrumbBuffer {
    cap;
    items = [];
    constructor(cap = DEFAULT_CAP) {
        this.cap = cap;
    }
    push(type, data = {}) {
        this.items.push({
            data,
            timestamp: new Date().toISOString(),
            type,
        });
        while (this.items.length > this.cap) {
            this.items.shift();
        }
    }
    snapshot() {
        return this.items.slice();
    }
    clear() {
        this.items.length = 0;
    }
}
const _global = new BreadcrumbBuffer();
export function addBreadcrumb(type, data = {}) {
    _global.push(type, data);
}
export function getBreadcrumbs() {
    return _global.snapshot();
}
export function clearBreadcrumbs() {
    _global.clear();
}
//# sourceMappingURL=breadcrumbs.js.map