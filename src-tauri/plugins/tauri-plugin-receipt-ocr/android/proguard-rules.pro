# ML Kit AARs ship their own consumer rules; these are a safety net for R8 release builds.
-keep class com.google.mlkit.** { *; }
-dontwarn com.google.mlkit.**
