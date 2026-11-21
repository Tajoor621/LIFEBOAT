package com.tajoor621.lifeboat

import android.Manifest
import android.content.ContentResolver
import android.content.Context
import android.content.pm.PackageManager
import android.provider.ContactsContract
import androidx.core.content.ContextCompat
import java.io.File
import java.io.FileOutputStream
import java.io.OutputStreamWriter
import java.security.MessageDigest
import java.util.concurrent.atomic.AtomicBoolean

data class ContactsSnapshotEvidence(
    val startedAtEpochMs: Long,
    val completedAtEpochMs: Long,
    val providerContactCount: Int,
    val exportedContactCount: Int,
    val dataRowsRead: Int,
    val outputBytes: Long,
    val sha256: String,
    val completion: String = "COMPLETE"
)

class ContactsSnapshotReader(private val context: Context) {
    fun createVcf(destination: File, cancelRequested: AtomicBoolean = AtomicBoolean(false)): ContactsSnapshotEvidence {
        requireContactsPermission()
        val started = System.currentTimeMillis()
        val resolver = context.contentResolver
        val providerCount = countContacts(resolver)
        val projection = arrayOf(
            ContactsContract.Data.CONTACT_ID, ContactsContract.Data.MIMETYPE,
            ContactsContract.Data.DATA1, ContactsContract.Data.DATA2,
            ContactsContract.Data.DATA3, ContactsContract.Data.DATA4,
            ContactsContract.Data.DATA5, ContactsContract.Data.DATA6,
            ContactsContract.Data.DATA7, ContactsContract.Data.DATA8,
            ContactsContract.Data.DATA9, ContactsContract.Data.DATA10
        )
        val mimeTypes = arrayOf(
            ContactsContract.CommonDataKinds.StructuredName.CONTENT_ITEM_TYPE,
            ContactsContract.CommonDataKinds.Phone.CONTENT_ITEM_TYPE,
            ContactsContract.CommonDataKinds.Email.CONTENT_ITEM_TYPE,
            ContactsContract.CommonDataKinds.Organization.CONTENT_ITEM_TYPE,
            ContactsContract.CommonDataKinds.StructuredPostal.CONTENT_ITEM_TYPE
        )
        val selection = "${ContactsContract.Data.MIMETYPE} IN (?,?,?,?,?)"
        val counts = intArrayOf(0, 0)
        destination.parentFile?.mkdirs()
        try {
            val cursor = resolver.query(
                ContactsContract.Data.CONTENT_URI, projection, selection, mimeTypes,
                "${ContactsContract.Data.CONTACT_ID} ASC"
            ) ?: error("Contacts provider returned no cursor")
            cursor.use { rows ->
                FileOutputStream(destination).buffered().use { stream ->
                    OutputStreamWriter(stream, Charsets.UTF_8).buffered().use { writer ->
                        var activeId: String? = null
                        var formatted = ""
                        var given = ""
                        var family = ""
                        val phones = mutableListOf<String>()
                        val emails = mutableListOf<String>()
                        val organizations = mutableListOf<String>()
                        val addresses = mutableListOf<String>()

                        fun flushContact() {
                            if (activeId == null) return
                            VCardWriter.writeContact(writer, VCardContact(
                                formatted, given, family, phones.toList(),
                                emails.toList(), organizations.toList(), addresses.toList()
                            ))
                            counts[0]++
                        }

                        while (rows.moveToNext()) {
                            check(!cancelRequested.get()) { "Contacts export stopped by owner" }
                            requireContactsPermission()
                            counts[1]++
                            val id = rows.getString(0) ?: continue
                            if (activeId != id) {
                                flushContact()
                                activeId = id
                                formatted = ""; given = ""; family = ""
                                phones.clear(); emails.clear(); organizations.clear(); addresses.clear()
                            }
                            when (rows.getString(1)) {
                                ContactsContract.CommonDataKinds.StructuredName.CONTENT_ITEM_TYPE -> {
                                    formatted = rows.getString(2).orEmpty()
                                    // DATA2 is given name; DATA3 is family name.
                                    given = rows.getString(3).orEmpty()
                                    family = rows.getString(4).orEmpty()
                                }
                                ContactsContract.CommonDataKinds.Phone.CONTENT_ITEM_TYPE ->
                                    rows.getString(2)?.takeIf(String::isNotBlank)?.let(phones::add)
                                ContactsContract.CommonDataKinds.Email.CONTENT_ITEM_TYPE ->
                                    rows.getString(2)?.takeIf(String::isNotBlank)?.let(emails::add)
                                ContactsContract.CommonDataKinds.Organization.CONTENT_ITEM_TYPE -> {
                                    val value = listOf(rows.getString(2).orEmpty(), rows.getString(5).orEmpty())
                                        .filter(String::isNotBlank).joinToString(" — ")
                                    if (value.isNotBlank()) organizations.add(value)
                                }
                                ContactsContract.CommonDataKinds.StructuredPostal.CONTENT_ITEM_TYPE -> {
                                    val value = listOf(2, 5, 8, 9, 10, 11)
                                        .map { rows.getString(it).orEmpty() }
                                        .filter(String::isNotBlank).joinToString(", ")
                                    if (value.isNotBlank()) addresses.add(value)
                                }
                            }
                        }
                        flushContact()
                    }
                }
            }
            requireContactsPermission()
            val digest = MessageDigest.getInstance("SHA-256")
            destination.inputStream().buffered().use { input ->
                val buffer = ByteArray(64 * 1024)
                while (true) {
                    check(!cancelRequested.get()) { "Contacts export stopped by owner" }
                    val read = input.read(buffer)
                    if (read < 0) break
                    digest.update(buffer, 0, read)
                }
            }
            return ContactsSnapshotEvidence(
                started, System.currentTimeMillis(), providerCount, counts[0], counts[1],
                destination.length(), digest.digest().joinToString("") { "%02x".format(it) }
            )
        } catch (failure: Exception) {
            destination.delete()
            throw failure
        }
    }

    private fun requireContactsPermission() {
        check(ContextCompat.checkSelfPermission(context, Manifest.permission.READ_CONTACTS) ==
            PackageManager.PERMISSION_GRANTED) { "Contacts permission is not granted or was revoked" }
    }

    private fun countContacts(resolver: ContentResolver): Int {
        val cursor = resolver.query(
            ContactsContract.Contacts.CONTENT_URI, arrayOf(ContactsContract.Contacts._ID),
            null, null, null
        ) ?: error("Contacts provider returned no count cursor")
        cursor.use { return it.count }
    }
}
