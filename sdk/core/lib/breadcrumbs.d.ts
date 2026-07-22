import type { Breadcrumb, BreadcrumbType } from './types.js';
export declare class BreadcrumbBuffer {
    private readonly cap;
    private readonly items;
    constructor(cap?: number);
    push(type: BreadcrumbType, data?: Record<string, unknown>): void;
    snapshot(): Breadcrumb[];
    clear(): void;
}
export declare function addBreadcrumb(type: BreadcrumbType, data?: Record<string, unknown>): void;
export declare function getBreadcrumbs(): Breadcrumb[];
export declare function clearBreadcrumbs(): void;
//# sourceMappingURL=breadcrumbs.d.ts.map