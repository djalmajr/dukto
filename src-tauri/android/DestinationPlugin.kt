package app.dukto

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Environment
import android.os.ParcelFileDescriptor
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import android.system.Os
import androidx.activity.result.ActivityResult
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.util.UUID

@InvokeArg
class PickFilesArgs {
  var multiple: Boolean = true
}

@InvokeArg
class ReleaseFilesArgs {
  lateinit var paths: Array<String>
}

@TauriPlugin
class DestinationPlugin(private val activity: Activity) : Plugin(activity) {
  private data class PendingSelection(
    val invoke: Invoke,
    val response: JSObject? = null,
    val error: String? = null,
  )

  private var pendingSelection: PendingSelection? = null
  private val selectedFileDescriptors = mutableMapOf<String, ParcelFileDescriptor>()
  private val selectedFilesRoot = File(activity.cacheDir, "dukto-selected-files")

  init {
    selectedFilesRoot.deleteRecursively()
    selectedFilesRoot.mkdirs()
  }

  @Command
  fun pickDirectory(invoke: Invoke) {
    val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
      addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
      addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
      addFlags(Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
      addFlags(Intent.FLAG_GRANT_PREFIX_URI_PERMISSION)
    }
    startActivityForResult(invoke, intent, "directoryPickerResult")
  }

  @Command
  fun pickFiles(invoke: Invoke) {
    val args = invoke.parseArgs(PickFilesArgs::class.java)
    val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
      addCategory(Intent.CATEGORY_OPENABLE)
      addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
      addFlags(Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
      type = "*/*"
      putExtra(Intent.EXTRA_ALLOW_MULTIPLE, args.multiple)
    }
    startActivityForResult(invoke, intent, "filePickerResult")
  }

  @ActivityCallback
  fun filePickerResult(invoke: Invoke, result: ActivityResult) {
    if (result.resultCode == Activity.RESULT_CANCELED) {
      completeWhenResumed(invoke, JSObject().apply { put("files", null) })
      return
    }
    if (result.resultCode != Activity.RESULT_OK) {
      completeWhenResumed(invoke, error = "Could not select files")
      return
    }

    val createdPaths = mutableListOf<String>()
    try {
      val data = result.data ?: throw IllegalStateException("File picker returned no data")
      val uris = mutableListOf<Uri>()
      data.clipData?.let { clipData ->
        for (index in 0 until clipData.itemCount) uris.add(clipData.getItemAt(index).uri)
      } ?: data.data?.let(uris::add)
      if (uris.isEmpty()) throw IllegalStateException("File picker returned no files")

      val files = JSArray()
      for (uri in uris) {
        tryPersistReadPermission(uri, data.flags)
        val file = materializeSelectedFile(uri)
        createdPaths.add(file.getString("path"))
        files.put(file)
      }
      completeWhenResumed(invoke, JSObject().apply { put("files", files) })
    } catch (error: Exception) {
      for (path in createdPaths) releaseSelectedFile(path)
      completeWhenResumed(invoke, error = error.message ?: "Could not use the selected files")
    }
  }

  @Command
  fun releaseFiles(invoke: Invoke) {
    val args = invoke.parseArgs(ReleaseFilesArgs::class.java)
    for (path in args.paths) releaseSelectedFile(path)
    invoke.resolve()
  }

  @Command
  fun openPrivacyPolicy(invoke: Invoke) {
    try {
      val intent = Intent(
        Intent.ACTION_VIEW,
        Uri.parse("https://dukto.app/docs/privacidade/"),
      )
      activity.startActivity(intent)
      invoke.resolve()
    } catch (error: Exception) {
      invoke.reject(error.message ?: "Could not open the privacy policy")
    }
  }

  @ActivityCallback
  fun directoryPickerResult(invoke: Invoke, result: ActivityResult) {
    if (result.resultCode == Activity.RESULT_CANCELED) {
      completeWhenResumed(invoke, JSObject().apply { put("path", null) })
      return
    }
    if (result.resultCode != Activity.RESULT_OK) {
      completeWhenResumed(invoke, error = "Could not select the destination folder")
      return
    }

    try {
      val data = result.data ?: throw IllegalStateException("Folder picker returned no data")
      val uri = data.data ?: throw IllegalStateException("Folder picker returned no URI")
      persistPermission(uri, data.flags)
      val directory = resolveExternalStoragePath(uri)
      verifyWritable(directory)
      completeWhenResumed(invoke, JSObject().apply { put("path", directory.canonicalPath) })
    } catch (error: Exception) {
      completeWhenResumed(
        invoke,
        error = error.message ?: "Could not use the selected destination folder",
      )
    }
  }

  override fun onResume() {
    activity.window.decorView.post { deliverPendingSelection() }
  }

  private fun completeWhenResumed(
    invoke: Invoke,
    response: JSObject? = null,
    error: String? = null,
  ) {
    pendingSelection = PendingSelection(invoke, response, error)
    val lifecycle = (activity as LifecycleOwner).lifecycle
    if (lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) {
      activity.window.decorView.post { deliverPendingSelection() }
    }
  }

  private fun deliverPendingSelection() {
    val pending = pendingSelection ?: return
    pendingSelection = null
    if (pending.error != null) pending.invoke.reject(pending.error)
    else pending.invoke.resolve(pending.response)
  }

  private fun persistPermission(uri: Uri, resultFlags: Int) {
    val accessFlags = resultFlags and
      (Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
    activity.contentResolver.takePersistableUriPermission(uri, accessFlags)
  }

  private fun tryPersistReadPermission(uri: Uri, resultFlags: Int) {
    try {
      val readFlag = resultFlags and Intent.FLAG_GRANT_READ_URI_PERMISSION
      if (readFlag != 0) activity.contentResolver.takePersistableUriPermission(uri, readFlag)
    } catch (_: SecurityException) {
      // Some document providers grant access for the process lifetime only.
    }
  }

  private fun materializeSelectedFile(uri: Uri): JSObject {
    val displayName = queryDisplayName(uri)
    val directory = File(selectedFilesRoot, UUID.randomUUID().toString())
    if (!directory.mkdirs()) throw IllegalStateException("Could not prepare the selected file")
    val link = File(directory, sanitizeFileName(displayName))
    val descriptor = activity.contentResolver.openFileDescriptor(uri, "r")
      ?: throw IllegalStateException("Could not open $displayName")
    val fileDescriptor = descriptor.fd

    try {
      Os.symlink("/proc/self/fd/$fileDescriptor", link.absolutePath)
      selectedFileDescriptors[link.absolutePath] = descriptor
      val size = queryFileSize(uri) ?: Os.fstat(descriptor.fileDescriptor).st_size
      return JSObject().apply {
        put("name", displayName)
        put("path", link.absolutePath)
        put("size", size)
        put("is_dir", false)
      }
    } catch (error: Exception) {
      descriptor.close()
      directory.deleteRecursively()
      throw error
    }
  }

  private fun queryDisplayName(uri: Uri): String {
    val projection = arrayOf(OpenableColumns.DISPLAY_NAME)
    activity.contentResolver.query(uri, projection, null, null, null)?.use { cursor ->
      if (cursor.moveToFirst()) {
        val index = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
        if (index >= 0 && !cursor.isNull(index)) {
          val name = cursor.getString(index)?.trim()
          if (!name.isNullOrEmpty()) return name
        }
      }
    }
    return uri.lastPathSegment?.substringAfterLast('/')?.ifBlank { "document" } ?: "document"
  }

  private fun queryFileSize(uri: Uri): Long? {
    val projection = arrayOf(OpenableColumns.SIZE)
    activity.contentResolver.query(uri, projection, null, null, null)?.use { cursor ->
      if (cursor.moveToFirst()) {
        val index = cursor.getColumnIndex(OpenableColumns.SIZE)
        if (index >= 0 && !cursor.isNull(index)) return cursor.getLong(index)
      }
    }
    return null
  }

  private fun sanitizeFileName(name: String): String {
    val sanitized = name.map { character ->
      if (character == '/' || character == '\\' || character == '\u0000') '_' else character
    }.joinToString("").trim().take(120)
    return sanitized.takeUnless { it.isEmpty() || it == "." || it == ".." } ?: "document"
  }

  private fun releaseSelectedFile(path: String) {
    val descriptor = selectedFileDescriptors.remove(path) ?: return
    try {
      descriptor.close()
    } finally {
      val link = File(path)
      link.delete()
      link.parentFile?.delete()
    }
  }

  private fun resolveExternalStoragePath(uri: Uri): File {
    if (uri.authority != "com.android.externalstorage.documents") {
      throw IllegalArgumentException("Select a folder in the device storage")
    }

    val documentId = DocumentsContract.getTreeDocumentId(uri)
    val parts = documentId.split(":", limit = 2)
    val volume = parts.firstOrNull().orEmpty()
    val relativePath = parts.getOrNull(1).orEmpty()
    val root = if (volume.equals("primary", ignoreCase = true)) {
      Environment.getExternalStorageDirectory()
    } else {
      File("/storage", volume)
    }
    return if (relativePath.isEmpty()) root else File(root, relativePath)
  }

  private fun verifyWritable(directory: File) {
    if (!directory.isDirectory) {
      throw IllegalArgumentException("The selected destination is not a folder")
    }
    val probe = File.createTempFile(".dukto-write-check-", ".tmp", directory)
    if (!probe.delete()) probe.deleteOnExit()
  }
}
