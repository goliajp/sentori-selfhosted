import { clearBreadcrumbs, getBreadcrumbs } from '@goliapkg/sentori-core';
import type { BreadcrumbType } from './types.js';
export type AddBreadcrumbInput = {
    data?: Record<string, unknown>;
    type: BreadcrumbType;
};
export declare function addBreadcrumb(input: AddBreadcrumbInput): void;
export { clearBreadcrumbs, getBreadcrumbs };
//# sourceMappingURL=breadcrumbs.d.ts.map