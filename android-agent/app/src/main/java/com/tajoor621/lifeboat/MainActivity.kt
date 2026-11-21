package com.tajoor621.lifeboat

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.provider.DocumentsContract
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Bundle
import android.view.ViewGroup
import android.widget.Button
import android.widget.CheckBox
import android.widget.LinearLayout
import android.widget.ImageView
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import androidx.core.content.ContextCompat
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

/** Visible, owner-initiated contacts export. No background collection or network access. */
class MainActivity : Activity() {
    private val preferences by lazy { getSharedPreferences("sentinel_consent_v1", MODE_PRIVATE) }
    private val executor = Executors.newSingleThreadExecutor()
    private val cancelRequested = AtomicBoolean(false)
    private val choices = linkedMapOf<DataClass, CheckBox>()
    private lateinit var confirmation: CheckBox
    private lateinit var status: TextView
    private lateinit var cancelButton: Button
    private var pendingVcf: File? = null
    private var pendingEvidence: ContactsSnapshotEvidence? = null
    private var askedContactsBefore = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val page = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(28, 32, 28, 32) }
        page.addView(ImageView(this).apply {
            setImageResource(R.drawable.ic_lifeboat_mark)
            contentDescription = "LIFEBOAT shield and rescue mark"
            scaleType = ImageView.ScaleType.FIT_CENTER
            layoutParams = LinearLayout.LayoutParams(112, 112).apply { gravity = android.view.Gravity.CENTER_HORIZONTAL }
        })
        page.addView(TextView(this).apply {
            text = "LIFEBOAT"
            textSize = 28f
            setTextColor(android.graphics.Color.rgb(16, 105, 190))
            gravity = android.view.Gravity.CENTER_HORIZONTAL
            setPadding(0, 4, 0, 0)
        })
        page.addView(TextView(this).apply {
            text = "BACKUP  •  RECOVER  •  BE READY"
            textSize = 11f
            letterSpacing = 0.12f
            gravity = android.view.Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, 18)
        })
        page.addView(TextView(this).apply {
            text = "Your data stays under your control"; textSize = 17f; setPadding(0, 8, 0, 20)
        })
        page.addView(TextView(this).apply {
            text = "Choose data categories. Saving consent never starts collection. Contacts are read only after you save Contacts consent and separately press the export action."
            textSize = 15f; setPadding(0, 0, 0, 18)
        })
        addChoice(page, DataClass.CONTACTS, "Contacts", "Read-only contacts export to a location you choose.")
        addChoice(page, DataClass.PHOTOS, "Photos", "Not implemented in this build.")
        addChoice(page, DataClass.VIDEOS, "Videos", "Not implemented in this build.")
        addChoice(page, DataClass.DOCUMENTS, "Documents", "Not implemented in this build.")
        confirmation = CheckBox(this).apply {
            text = "I understand these categories and explicitly approve saving my choices."
            textSize = 14f; isChecked = preferences.getBoolean(KEY_CONFIRMED, false); setPadding(0, 16, 0, 12)
        }
        page.addView(confirmation)
        page.addView(Button(this).apply { text = "Save consent choices"; setOnClickListener { saveConsent() } }, matchWidth())
        page.addView(Button(this).apply {
            text = "Connect Contacts and export VCF"; setOnClickListener { beginContactsExport() }
        }, matchWidth())
        page.addView(Button(this).apply {
            text = "Withdraw consent and clear choices"; setOnClickListener { withdrawConsent() }
        }, matchWidth())
        cancelButton = Button(this).apply {
            text = "Stop current export"
            isEnabled = false
            setOnClickListener { cancelExport() }
        }
        page.addView(cancelButton, matchWidth())
        status = TextView(this).apply { textSize = 13f; setPadding(0, 18, 0, 0) }
        page.addView(status)
        refreshStatus()
        setContentView(ScrollView(this).apply { addView(page) })
    }

    private fun addChoice(parent: LinearLayout, dataClass: DataClass, title: String, explanation: String) {
        val group = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(0, 8, 0, 8) }
        val checkBox = CheckBox(this).apply {
            text = title; textSize = 17f; isChecked = preferences.getBoolean(keyFor(dataClass), false)
        }
        choices[dataClass] = checkBox
        group.addView(checkBox)
        group.addView(TextView(this).apply { text = explanation; textSize = 13f; setPadding(42, 0, 0, 0) })
        parent.addView(group)
    }

    private fun saveConsent() {
        val selected = choices.filterValues { it.isChecked }.keys
        if (!confirmation.isChecked || selected.isEmpty()) {
            preferences.edit().clear().apply()
            refreshStatus()
            Toast.makeText(this, "No consent saved. Confirm and select at least one category.", Toast.LENGTH_LONG).show()
            return
        }
        val editor = preferences.edit().putBoolean(KEY_CONFIRMED, true)
        DataClass.values().forEach { editor.putBoolean(keyFor(it), it in selected) }
        editor.apply()
        refreshStatus()
        Toast.makeText(this, "Consent preferences saved on this device.", Toast.LENGTH_SHORT).show()
    }

    private fun withdrawConsent() {
        cancelRequested.set(true)
        cancelButton.isEnabled = false
        preferences.edit().clear().apply()
        choices.values.forEach { it.isChecked = false }
        confirmation.isChecked = false
        pendingVcf?.delete(); pendingVcf = null; pendingEvidence = null
        refreshStatus()
        Toast.makeText(this, "Consent withdrawn; saved choices cleared.", Toast.LENGTH_SHORT).show()
    }

    private fun beginContactsExport() {
        val selectedNow = choices.filterValues { it.isChecked }.keys
        val savedSelected = DataClass.values().filter { preferences.getBoolean(keyFor(it), false) }.toSet()
        val savedConfirmed = preferences.getBoolean(KEY_CONFIRMED, false)
        if (!confirmation.isChecked || !savedConfirmed || selectedNow != savedSelected ||
            DataClass.CONTACTS !in selectedNow) {
            status.text = "Your current choices differ from saved consent. Confirm the selection, then press Save consent choices before exporting."
            return
        }
        cancelRequested.set(false)
        cancelButton.isEnabled = true
        if (ContextCompat.checkSelfPermission(this, Manifest.permission.READ_CONTACTS) == PackageManager.PERMISSION_GRANTED) {
            createContactsSnapshot()
            return
        }
        askedContactsBefore = preferences.getBoolean(KEY_CONTACTS_ASKED, false)
        preferences.edit().putBoolean(KEY_CONTACTS_ASKED, true).apply()
        requestPermissions(arrayOf(Manifest.permission.READ_CONTACTS), REQUEST_CONTACTS)
    }

    @Deprecated("Platform permission callback retained for the minimal activity shell.")
    override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<out String>, grantResults: IntArray) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode != REQUEST_CONTACTS) return
        if (grantResults.firstOrNull() == PackageManager.PERMISSION_GRANTED) {
            createContactsSnapshot()
        } else {
            val state = if (askedContactsBefore && !shouldShowRequestPermissionRationale(Manifest.permission.READ_CONTACTS))
                "PERMANENTLY_DENIED" else "DENIED"
            cancelButton.isEnabled = false
            status.text = "Contacts permission: $state. No contacts were read."
        }
    }

    private fun createContactsSnapshot() {
        cancelButton.isEnabled = true
        status.text = "Reading contacts with your permission. You can stop this export at any time."
        executor.execute {
            val file = File(cacheDir, "lifeboat-contacts-${System.currentTimeMillis()}.vcf")
            try {
                val evidence = ContactsSnapshotReader(this).createVcf(file, cancelRequested)
                if (cancelRequested.get() || isFinishing || isDestroyed) {
                    file.delete()
                    return@execute
                }
                runOnUiThread {
                    if (isFinishing || isDestroyed || cancelRequested.get()) {
                        file.delete()
                        cancelButton.isEnabled = false
                        return@runOnUiThread
                    }
                    pendingVcf = file
                    pendingEvidence = evidence
                    status.text = "Snapshot ready: ${evidence.exportedContactCount} vCards from ${evidence.providerContactCount} provider contacts; ${evidence.outputBytes} bytes. Choose where to save the VCF."
                    val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
                        addCategory(Intent.CATEGORY_OPENABLE); type = "text/vcard"
                        putExtra(Intent.EXTRA_TITLE, "LIFEBOAT-contacts.vcf")
                    }
                    startActivityForResult(intent, REQUEST_CREATE_DOCUMENT)
                }
            } catch (failure: Exception) {
                file.delete()
                if (!isFinishing && !isDestroyed) runOnUiThread {
                    cancelButton.isEnabled = false
                    status.text = if (cancelRequested.get()) "Export stopped. Temporary contacts data was deleted."
                    else "Contacts snapshot failed: ${failure.message ?: "provider or permission error"}. No partial export was kept."
                }
            }
        }
    }

    @Deprecated("Document picker result handled through platform activity result.")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode != REQUEST_CREATE_DOCUMENT) return
        val source = pendingVcf
        val snapshot = pendingEvidence
        pendingVcf = null; pendingEvidence = null
        val uri: Uri? = if (resultCode == RESULT_OK) data?.data else null
        if (source == null || snapshot == null || uri == null || cancelRequested.get()) {
            source?.delete()
            cancelButton.isEnabled = false
            status.text = "Export cancelled. Temporary contacts data was deleted."
            return
        }
        executor.execute {
            try {
                val output = contentResolver.openOutputStream(uri) ?: error("Could not open selected destination")
                source.inputStream().buffered().use { input ->
                    output.buffered().use { target ->
                        val buffer = ByteArray(64 * 1024)
                        while (true) {
                            check(!cancelRequested.get()) { "Export stopped by owner" }
                            val count = input.read(buffer)
                            if (count < 0) break
                            target.write(buffer, 0, count)
                        }
                        target.flush()
                    }
                }
                val destinationDigest = MessageDigest.getInstance("SHA-256")
                var destinationBytes = 0L
                val copied = contentResolver.openInputStream(uri) ?: error("Could not verify selected destination")
                copied.buffered().use { input ->
                    val buffer = ByteArray(64 * 1024)
                    while (true) {
                        check(!cancelRequested.get()) { "Export stopped by owner" }
                        val count = input.read(buffer)
                        if (count < 0) break
                        destinationDigest.update(buffer, 0, count)
                        destinationBytes += count
                    }
                }
                val destinationHash = destinationDigest.digest().joinToString("") { "%02x".format(it) }
                check(destinationBytes == snapshot.outputBytes && destinationHash == snapshot.sha256) {
                    "Destination integrity verification failed"
                }
                val evidenceFile = File(filesDir, "contacts-evidence-${snapshot.completedAtEpochMs}.json")
                val json = """{"startedAtEpochMs":${snapshot.startedAtEpochMs},"completedAtEpochMs":${snapshot.completedAtEpochMs},"providerContactCount":${snapshot.providerContactCount},"exportedContactCount":${snapshot.exportedContactCount},"dataRowsRead":${snapshot.dataRowsRead},"outputBytes":${snapshot.outputBytes},"sha256":"${snapshot.sha256}","completion":"EXPORTED_VERIFIED"}"""
                evidenceFile.writeText(json, Charsets.UTF_8)
                source.delete()
                if (!isFinishing && !isDestroyed) runOnUiThread {
                    cancelButton.isEnabled = false
                    status.text = "VCF export saved. Provider contacts: ${snapshot.providerContactCount}; exported cards: ${snapshot.exportedContactCount}; bytes: ${snapshot.outputBytes}. Private evidence and SHA-256 saved."
                }
            } catch (failure: Exception) {
                source.delete()
                try { DocumentsContract.deleteDocument(contentResolver, uri) } catch (_: Exception) { /* Provider may not support deletion. */ }
                if (!isFinishing && !isDestroyed) runOnUiThread {
                    cancelButton.isEnabled = false
                    status.text = if (cancelRequested.get()) "Export stopped. Temporary data was deleted; destination cleanup was attempted."
                    else "Could not save the selected VCF destination: ${failure.message ?: "write error"}. Temporary data deleted."
                }
            }
        }
    }

    private fun cancelExport() {
        cancelRequested.set(true)
        pendingVcf?.delete()
        pendingVcf = null
        pendingEvidence = null
        cancelButton.isEnabled = false
        status.text = "Stop requested. The current operation will stop at its next safe checkpoint."
    }

    private fun refreshStatus() {
        val consent = SnapshotConsent(
            preferences.getBoolean(KEY_CONFIRMED, false),
            DataClass.values().filter { preferences.getBoolean(keyFor(it), false) }.toSet()
        )
        status.text = if (consent.hasExplicitScope())
            "Consent preferences saved. No collection runs automatically. Contacts require the separate export action."
        else "No active consent preferences. No data is collected."
    }

    private fun keyFor(dataClass: DataClass) = "scope_${dataClass.name.lowercase()}"
    private fun matchWidth() = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)

    override fun onDestroy() {
        cancelRequested.set(true)
        pendingVcf?.delete()
        pendingVcf = null
        executor.shutdownNow()
        super.onDestroy()
    }

    companion object {
        private const val KEY_CONFIRMED = "explicitly_confirmed"
        private const val KEY_CONTACTS_ASKED = "contacts_permission_asked"
        private const val REQUEST_CONTACTS = 621
        private const val REQUEST_CREATE_DOCUMENT = 622
    }
}
