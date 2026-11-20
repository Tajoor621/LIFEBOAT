package com.tajoor621.lifeboat

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Bundle
import android.view.ViewGroup
import android.widget.Button
import android.widget.CheckBox
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import androidx.core.content.ContextCompat
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.Executors

/** Visible, owner-initiated contacts export. No background collection or network access. */
class MainActivity : Activity() {
    private val preferences by lazy { getSharedPreferences("sentinel_consent_v1", MODE_PRIVATE) }
    private val executor = Executors.newSingleThreadExecutor()
    private val choices = linkedMapOf<DataClass, CheckBox>()
    private lateinit var confirmation: CheckBox
    private lateinit var status: TextView
    private var pendingVcf: File? = null
    private var pendingEvidence: ContactsSnapshotEvidence? = null
    private var askedContactsBefore = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val page = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(28, 32, 28, 32) }
        page.addView(TextView(this).apply { text = "LIFEBOAT Sentinel"; textSize = 26f })
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
        preferences.edit().clear().apply()
        choices.values.forEach { it.isChecked = false }
        confirmation.isChecked = false
        pendingVcf?.delete(); pendingVcf = null; pendingEvidence = null
        refreshStatus()
        Toast.makeText(this, "Consent withdrawn; saved choices cleared.", Toast.LENGTH_SHORT).show()
    }

    private fun beginContactsExport() {
        val consent = SnapshotConsent(
            preferences.getBoolean(KEY_CONFIRMED, false),
            DataClass.values().filter { preferences.getBoolean(keyFor(it), false) }.toSet()
        )
        if (!consent.hasExplicitScope() || DataClass.CONTACTS !in consent.selectedClasses) {
            status.text = "Save explicit Contacts consent before starting a contacts export."
            return
        }
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
            status.text = "Contacts permission: $state. No contacts were read."
        }
    }

    private fun createContactsSnapshot() {
        status.text = "Reading contacts with your permission. This may take time for a large address book."
        executor.execute {
            val file = File(cacheDir, "lifeboat-contacts-${System.currentTimeMillis()}.vcf")
            try {
                val evidence = ContactsSnapshotReader(this).createVcf(file)
                runOnUiThread {
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
                runOnUiThread { status.text = "Contacts snapshot failed: ${failure.message ?: "provider or permission error"}. No partial export was kept." }
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
        if (source == null || snapshot == null || uri == null) {
            source?.delete()
            status.text = "Export cancelled. Temporary contacts data was deleted."
            return
        }
        executor.execute {
            try {
                val output = contentResolver.openOutputStream(uri) ?: error("Could not open selected destination")
                source.inputStream().buffered().use { input ->
                    output.buffered().use { target -> input.copyTo(target) }
                }
                val evidenceFile = File(filesDir, "contacts-evidence-${snapshot.completedAtEpochMs}.json")
                val json = """{"startedAtEpochMs":${snapshot.startedAtEpochMs},"completedAtEpochMs":${snapshot.completedAtEpochMs},"providerContactCount":${snapshot.providerContactCount},"exportedContactCount":${snapshot.exportedContactCount},"dataRowsRead":${snapshot.dataRowsRead},"outputBytes":${snapshot.outputBytes},"sha256":"${snapshot.sha256}","completion":"EXPORTED"}"""
                evidenceFile.writeText(json, Charsets.UTF_8)
                source.delete()
                runOnUiThread {
                    status.text = "VCF export saved. Provider contacts: ${snapshot.providerContactCount}; exported cards: ${snapshot.exportedContactCount}; bytes: ${snapshot.outputBytes}. Private evidence and SHA-256 saved."
                }
            } catch (failure: Exception) {
                source.delete()
                runOnUiThread { status.text = "Could not save the selected VCF destination: ${failure.message ?: "write error"}. Temporary data deleted." }
            }
        }
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
        executor.shutdown()
        super.onDestroy()
    }

    companion object {
        private const val KEY_CONFIRMED = "explicitly_confirmed"
        private const val KEY_CONTACTS_ASKED = "contacts_permission_asked"
        private const val REQUEST_CONTACTS = 621
        private const val REQUEST_CREATE_DOCUMENT = 622
    }
}
