package com.tajoor621.lifeboat

import android.app.Activity
import android.os.Bundle
import android.view.ViewGroup
import android.widget.Button
import android.widget.CheckBox
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast

/** P2.1 consent preferences only; this screen never reads device data. */
class MainActivity : Activity() {
    private val preferences by lazy {
        getSharedPreferences("sentinel_consent_v1", MODE_PRIVATE)
    }
    private val choices = linkedMapOf<DataClass, CheckBox>()
    private lateinit var confirmation: CheckBox
    private lateinit var status: TextView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val page = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(28, 32, 28, 32)
        }
        page.addView(TextView(this).apply {
            text = "LIFEBOAT Sentinel"
            textSize = 26f
        })
        page.addView(TextView(this).apply {
            text = "Your data stays under your control"
            textSize = 17f
            setPadding(0, 8, 0, 20)
        })
        page.addView(TextView(this).apply {
            text = "Choose the data categories you may want to back up. These choices are stored on this device. This build does not read, copy, upload, or modify personal data, and it does not request Android permissions."
            textSize = 15f
            setPadding(0, 0, 0, 18)
        })
        addChoice(page, DataClass.CONTACTS, "Contacts", "Contact details, if you later authorize access.")
        addChoice(page, DataClass.PHOTOS, "Photos", "Photo files you choose or explicitly authorize.")
        addChoice(page, DataClass.VIDEOS, "Videos", "Video files you choose or explicitly authorize.")
        addChoice(page, DataClass.DOCUMENTS, "Documents", "Only files or folders you explicitly select.")

        confirmation = CheckBox(this).apply {
            text = "I understand these categories and explicitly approve saving my choices."
            textSize = 14f
            isChecked = preferences.getBoolean(KEY_CONFIRMED, false)
            setPadding(0, 16, 0, 12)
        }
        page.addView(confirmation)
        page.addView(Button(this).apply {
            text = "Save consent choices"
            setOnClickListener { saveConsent() }
        }, matchWidth())
        page.addView(Button(this).apply {
            text = "Withdraw consent and clear choices"
            setOnClickListener { withdrawConsent() }
        }, matchWidth())
        status = TextView(this).apply {
            textSize = 13f
            setPadding(0, 18, 0, 0)
        }
        page.addView(status)
        refreshStatus()
        setContentView(ScrollView(this).apply { addView(page) })
    }

    private fun addChoice(parent: LinearLayout, dataClass: DataClass, title: String, explanation: String) {
        val group = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(0, 8, 0, 8)
        }
        val checkBox = CheckBox(this).apply {
            text = title
            textSize = 17f
            isChecked = preferences.getBoolean(keyFor(dataClass), false)
        }
        choices[dataClass] = checkBox
        group.addView(checkBox)
        group.addView(TextView(this).apply {
            text = explanation
            textSize = 13f
            setPadding(42, 0, 0, 0)
        })
        parent.addView(group)
    }

    private fun saveConsent() {
        val selected = choices.filterValues { it.isChecked }.keys
        if (!confirmation.isChecked || selected.isEmpty()) {
            Toast.makeText(this, "Confirm your understanding and select at least one category.", Toast.LENGTH_LONG).show()
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
        refreshStatus()
        Toast.makeText(this, "Consent withdrawn; saved choices cleared.", Toast.LENGTH_SHORT).show()
    }

    private fun refreshStatus() {
        val consent = SnapshotConsent(
            preferences.getBoolean(KEY_CONFIRMED, false),
            DataClass.values().filter { preferences.getBoolean(keyFor(it), false) }.toSet()
        )
        status.text = if (consent.mayStartSnapshot()) {
            "Consent preferences saved. No snapshot has started; collection is not implemented in this build."
        } else {
            "No active consent preferences. No data is collected."
        }
    }

    private fun keyFor(dataClass: DataClass) = "scope_${dataClass.name.lowercase()}"

    private fun matchWidth() = LinearLayout.LayoutParams(
        ViewGroup.LayoutParams.MATCH_PARENT,
        ViewGroup.LayoutParams.WRAP_CONTENT
    )

    companion object { private const val KEY_CONFIRMED = "explicitly_confirmed" }
}
