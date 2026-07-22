export declare const SentoriErrorBoundary: import("vue").DefineComponent<import("vue").ExtractPropTypes<{
    /** Optional list of error names (`error.name`) to ignore — they
     *  pass through to upper boundaries unchanged. */
    ignore: {
        type: () => readonly string[];
        default: () => never[];
    };
}>, () => import("vue").VNode<import("vue").RendererNode, import("vue").RendererElement, {
    [key: string]: any;
}> | import("vue").VNode<import("vue").RendererNode, import("vue").RendererElement, {
    [key: string]: any;
}>[] | undefined, {}, {}, {}, import("vue").ComponentOptionsMixin, import("vue").ComponentOptionsMixin, {}, string, import("vue").PublicProps, Readonly<import("vue").ExtractPropTypes<{
    /** Optional list of error names (`error.name`) to ignore — they
     *  pass through to upper boundaries unchanged. */
    ignore: {
        type: () => readonly string[];
        default: () => never[];
    };
}>> & Readonly<{}>, {
    ignore: readonly string[];
}, {}, {}, {}, string, import("vue").ComponentProvideOptions, true, {}, any>;
//# sourceMappingURL=ErrorBoundary.d.ts.map