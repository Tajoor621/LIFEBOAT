package com.tajoor621.lifeboat

import android.app.Activity
import android.os.Bundle
import android.view.Gravity
import android.widget.LinearLayout
import android.widget.TextView

/**
 * P2.0 visible shell only. It intentionally requests no permissions and collects no data.
 */
class MainActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val content = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            setPadding(32, 48, 32, 48)
        }

        content.addView(TextView(this).apply {
            text = "LIFEBOAT Sentinel"
            textSize = 26f
            gravity = Gravity.CENTER
        })

        content.addView(TextView(this).apply {
            text = "P2 foundation — no data is collected yet. Backup will only start after you review the scope and explicitly consent."
            textSize = 16f
            gravity = Gravity.CENTER
            setPadding(0, 20, 0, 0)
        })

        setContentView(content)
    }
}
