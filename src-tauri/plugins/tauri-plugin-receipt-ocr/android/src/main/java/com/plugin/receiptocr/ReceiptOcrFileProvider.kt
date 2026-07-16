package com.plugin.receiptocr

import androidx.core.content.FileProvider

// Distinct FileProvider subclass so the manifest merger doesn't collide with the
// app's own androidx FileProvider (merger keys <provider> entries by class name).
class ReceiptOcrFileProvider : FileProvider()
