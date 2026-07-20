/**
 * Subset of the `expo-application` module we read for release
 * derivation. Defining a structural type instead of importing the
 * module keeps `expo-application` a peer dep that the Expo runtime
 * provides — bare-RN consumers don't need to install it.
 */
export type ExpoApplicationLike = {
    /** e.g. "com.example.myapp" — Android applicationId / iOS bundleId. */
    applicationId?: null | string;
    /** e.g. "5" — Android versionCode / iOS CFBundleVersion. */
    nativeBuildVersion?: null | string;
    /** e.g. "1.2.3" — iOS CFBundleShortVersionString / Android versionName. */
    nativeApplicationVersion?: null | string;
};
export type InitOptions = {
    /** Pass `import * as Application from 'expo-application'` here. */
    application?: ExpoApplicationLike;
    /** Override the auto-derived environment. Defaults to dev/prod via __DEV__. */
    environment?: string;
    /** Override ingest URL (self-hosted). Defaults to public SaaS. */
    ingestUrl?: string;
    /** Manual release override — required when `application` is omitted. */
    release?: string;
    /** Project public token, format `st_pk_...`. */
    token: string;
};
//# sourceMappingURL=types.d.ts.map