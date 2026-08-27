# The SDK reflects on Firebase classes at runtime to no-op when
# push is not in the build; keep their names.
-keep class com.sentori.** { *; }
-dontwarn com.google.firebase.**
