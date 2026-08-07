export type LenientOutcome = {
    /** What failed, one line. */
    failure: string;
    /** What it means for the customer, one or two lines. */
    impact: string;
    /** A copy-pasteable retry command. */
    retry: string;
};
export declare const isStrict: (argv: string[]) => boolean;
/** Strip --strict before handing argv to parseArgs. */
export declare const stripStrict: (argv: string[]) => string[];
export declare const lenientFail: (strict: boolean, o: LenientOutcome) => number;
//# sourceMappingURL=lenient.d.ts.map