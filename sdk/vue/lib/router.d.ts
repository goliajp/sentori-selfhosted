type RouterLike = {
    beforeEach: (cb: (to: {
        path: string;
        name?: unknown;
    }, from: {
        path: string;
    }) => void) => void;
    afterEach: (cb: (to: {
        path: string;
        name?: unknown;
    }, from: {
        path: string;
    }) => void) => void;
};
export declare function setupTraceNavigation(router: RouterLike): void;
export {};
//# sourceMappingURL=router.d.ts.map