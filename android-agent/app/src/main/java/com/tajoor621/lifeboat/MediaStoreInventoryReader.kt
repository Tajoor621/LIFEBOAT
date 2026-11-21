package com.tajoor621.lifeboat

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.MediaStore
import androidx.core.content.ContextCompat
import java.util.concurrent.atomic.AtomicBoolean

data class MediaInventoryEvidence(val imageCount: Int, val videoCount: Int, val totalBytes: Long, val scope: String)

/** Read-only metadata inventory. Never opens media content or persists names, paths or URIs. */
class MediaStoreInventoryReader(private val context: Context) {
    fun inventory(includeImages: Boolean, includeVideos: Boolean, cancelled: AtomicBoolean = AtomicBoolean(false)): MediaInventoryEvidence {
        require(includeImages || includeVideos) { "No media category was selected" }
        val imageAccess = includeImages && hasAccess(DataClass.PHOTOS)
        val videoAccess = includeVideos && hasAccess(DataClass.VIDEOS)
        check(imageAccess || videoAccess) { "No selected media permission is currently granted" }
        val image = if (imageAccess) count(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, cancelled) else 0L to 0
        val video = if (videoAccess) count(MediaStore.Video.Media.EXTERNAL_CONTENT_URI, cancelled) else 0L to 0
        val full = mutableListOf<Boolean>()
        if (includeImages) full.add(isFullAccess(DataClass.PHOTOS))
        if (includeVideos) full.add(isFullAccess(DataClass.VIDEOS))
        return MediaInventoryEvidence(image.second, video.second, image.first + video.first, if (full.all { it }) "FULL" else "PARTIAL")
    }
    private fun hasAccess(dataClass: DataClass): Boolean =
        ContextCompat.checkSelfPermission(context, permissionFor(dataClass)) == PackageManager.PERMISSION_GRANTED ||
            (Build.VERSION.SDK_INT >= 34 && ContextCompat.checkSelfPermission(context, Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED) == PackageManager.PERMISSION_GRANTED)
    private fun isFullAccess(dataClass: DataClass): Boolean {
        if (Build.VERSION.SDK_INT < 33) return ContextCompat.checkSelfPermission(context, Manifest.permission.READ_EXTERNAL_STORAGE) == PackageManager.PERMISSION_GRANTED
        return ContextCompat.checkSelfPermission(context, permissionFor(dataClass)) == PackageManager.PERMISSION_GRANTED
    }
    private fun permissionFor(dataClass: DataClass): String = when {
        Build.VERSION.SDK_INT >= 33 && dataClass == DataClass.PHOTOS -> Manifest.permission.READ_MEDIA_IMAGES
        Build.VERSION.SDK_INT >= 33 && dataClass == DataClass.VIDEOS -> Manifest.permission.READ_MEDIA_VIDEO
        else -> Manifest.permission.READ_EXTERNAL_STORAGE
    }
    private fun count(uri: Uri, cancelled: AtomicBoolean): Pair<Long, Int> {
        check(!cancelled.get()) { "Media inventory stopped by owner" }
        val projection = arrayOf(MediaStore.MediaColumns._ID, MediaStore.MediaColumns.SIZE, MediaStore.MediaColumns.MIME_TYPE, MediaStore.MediaColumns.WIDTH, MediaStore.MediaColumns.HEIGHT)
        val cursor = context.contentResolver.query(uri, projection, null, null, null) ?: error("Media provider returned no cursor")
        var bytes = 0L
        var items = 0
        cursor.use { rows ->
            while (rows.moveToNext()) {
                check(!cancelled.get()) { "Media inventory stopped by owner" }
                val sizeIndex = rows.getColumnIndex(MediaStore.MediaColumns.SIZE)
                if (sizeIndex >= 0 && !rows.isNull(sizeIndex)) {
                    val size = rows.getLong(sizeIndex)
                    if (size > 0) bytes += size
                }
                items++
            }
        }
        return bytes to items
    }
}
