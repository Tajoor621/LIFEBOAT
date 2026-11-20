package com.tajoor621.lifeboat

import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.StringWriter

class VCardWriterTest {
    @Test fun writesUnicodeAndEscapesVCardText() {
        val output = StringWriter()
        VCardWriter.writeContact(output, VCardContact(
            formattedName = "نور، Noor; \\",
            givenName = "نور",
            familyName = "Gull",
            phones = listOf("+971, 50;123"),
            emails = listOf("noor@example.test"),
            organizations = listOf("LIFEBOAT")
        ))
        val value = output.toString()
        assertTrue(value.startsWith("BEGIN:VCARD\r\nVERSION:4.0\r\n"))
        assertTrue(value.contains("FN:نور\\, Noor\\; \\\\"))
        assertTrue(value.contains("TEL;VALUE=uri:tel:+971%2C%2050%3B123"))
        assertTrue(value.contains("EMAIL:noor@example.test"))
        assertTrue(value.endsWith("END:VCARD\r\n"))
    }

    @Test fun emitsEmptyContactWithSafeFallbackName() {
        val output = StringWriter()
        VCardWriter.writeContact(output, VCardContact())
        assertTrue(output.toString().contains("FN:Unnamed contact\r\n"))
    }
}
