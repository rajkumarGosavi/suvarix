package com.plugin.receiptocr

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import android.provider.MediaStore
import androidx.activity.result.ActivityResult
import androidx.core.content.FileProvider
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.text.TextRecognition
import com.google.mlkit.vision.text.latin.TextRecognizerOptions
import java.io.File

@InvokeArg
class ScanArgs {
    var source: String = "camera"
}

@TauriPlugin
class ReceiptOcrPlugin(private val activity: Activity) : Plugin(activity) {
    // Camera output file for the in-flight scan; null when picking from gallery.
    private var pendingCameraFile: File? = null

    private fun scanDir(): File = File(activity.cacheDir, "receipt_ocr").apply { mkdirs() }

    @Command
    fun scanReceipt(invoke: Invoke) {
        val args = invoke.parseArgs(ScanArgs::class.java)
        // Sweep leftovers from scans aborted by process death.
        scanDir().listFiles()?.forEach { it.delete() }
        pendingCameraFile = null
        try {
            val intent = if (args.source == "gallery") {
                Intent(Intent.ACTION_GET_CONTENT).apply { type = "image/*" }
            } else {
                val file = File.createTempFile("receipt_", ".jpg", scanDir())
                pendingCameraFile = file
                val uri = FileProvider.getUriForFile(
                    activity,
                    "${activity.packageName}.receiptocr.fileprovider",
                    file
                )
                Intent(MediaStore.ACTION_IMAGE_CAPTURE).apply {
                    putExtra(MediaStore.EXTRA_OUTPUT, uri)
                    addFlags(
                        Intent.FLAG_GRANT_READ_URI_PERMISSION or
                            Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                    )
                }
            }
            startActivityForResult(invoke, intent, "onScanResult")
        } catch (e: ActivityNotFoundException) {
            cleanup()
            invoke.reject("No app available to capture or pick an image")
        } catch (e: Exception) {
            cleanup()
            invoke.reject(e.message ?: "Failed to start receipt capture")
        }
    }

    @ActivityCallback
    fun onScanResult(invoke: Invoke, result: ActivityResult) {
        if (result.resultCode != Activity.RESULT_OK) {
            cleanup()
            invoke.resolve(cancelledResult())
            return
        }
        val uri: Uri? = pendingCameraFile?.let { Uri.fromFile(it) } ?: result.data?.data
        if (uri == null) {
            cleanup()
            invoke.reject("No image returned")
            return
        }
        try {
            val image = InputImage.fromFilePath(activity, uri)
            TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS)
                .process(image)
                .addOnSuccessListener { text ->
                    cleanup()
                    // ML Kit block order is not guaranteed top-down — sort by bounding box.
                    val sorted = text.textBlocks
                        .flatMap { it.lines }
                        .sortedBy { it.boundingBox?.top ?: 0 }
                    val lines = JSArray()
                    for (line in sorted) {
                        val o = JSObject()
                        o.put("text", line.text)
                        o.put("top", line.boundingBox?.top ?: 0)
                        lines.put(o)
                    }
                    val ret = JSObject()
                    ret.put("cancelled", false)
                    ret.put("fullText", sorted.joinToString("\n") { it.text })
                    ret.put("lines", lines)
                    invoke.resolve(ret)
                }
                .addOnFailureListener { e ->
                    cleanup()
                    invoke.reject("OCR failed: ${e.message}")
                }
        } catch (e: Exception) {
            cleanup()
            invoke.reject("Could not read image: ${e.message}")
        }
    }

    private fun cancelledResult(): JSObject {
        val ret = JSObject()
        ret.put("cancelled", true)
        ret.put("fullText", "")
        ret.put("lines", JSArray())
        return ret
    }

    private fun cleanup() {
        pendingCameraFile?.delete()
        pendingCameraFile = null
    }
}
