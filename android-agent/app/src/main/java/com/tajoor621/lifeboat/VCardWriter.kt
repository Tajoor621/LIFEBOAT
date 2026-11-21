package com.tajoor621.lifeboat

import java.io.Writer

data class VCardContact(
    val formattedName: String = "",
    val givenName: String = "",
    val familyName: String = "",
    val phones: List<String> = emptyList(),
    val emails: List<String> = emptyList(),
    val organizations: List<String> = emptyList(),
    val addresses: List<String> = emptyList()
)

object VCardWriter {
    fun writeContact(writer: Writer, contact: VCardContact) {
        writeLine(writer, "BEGIN:VCARD")
        writeLine(writer, "VERSION:4.0")
        val name = contact.formattedName.ifBlank {
            listOf(contact.givenName, contact.familyName).filter { it.isNotBlank() }.joinToString(" ")
        }
        writeLine(writer, "FN:${escape(name.ifBlank { "Unnamed contact" })}")
        writeLine(writer, "N:${escape(contact.familyName)};${escape(contact.givenName)};;;")
        contact.phones.filter(String::isNotBlank).distinct().forEach {
            writeLine(writer, "TEL;VALUE=uri:tel:${phoneUri(it)}")
        }
        contact.emails.filter(String::isNotBlank).distinct().forEach {
            writeLine(writer, "EMAIL:${escape(it)}")
        }
        contact.organizations.filter(String::isNotBlank).distinct().forEach {
            writeLine(writer, "ORG:${escape(it)}")
        }
        contact.addresses.filter(String::isNotBlank).distinct().forEach {
            writeLine(writer, "ADR:;;${escape(it)};;;;")
        }
        writeLine(writer, "END:VCARD")
    }

    /** RFC 6350 folding: each physical line is at most 75 UTF-8 octets. */
    private fun writeLine(writer: Writer, value: String) {
        var line = StringBuilder()
        var octets = 0
        var index = 0
        var firstPhysicalLine = true
        while (index < value.length) {
            val codePoint = value.codePointAt(index)
            val unit = String(Character.toChars(codePoint))
            val size = unit.toByteArray(Charsets.UTF_8).size
            val limit = if (firstPhysicalLine) 75 else 74
            if (octets + size > limit) {
                writer.append(line).append("\r\n")
                line = StringBuilder(" ")
                octets = 1
                firstPhysicalLine = false
            }
            line.append(unit)
            octets += size
            index += Character.charCount(codePoint)
        }
        writer.append(line).append("\r\n")
    }

    private fun phoneUri(value: String): String = value.toByteArray(Charsets.UTF_8).joinToString("") { byte ->
        val code = byte.toInt() and 0xff
        val char = code.toChar()
        if (code < 128 && (char.isLetterOrDigit() || char == '+' || char == '-' || char == '.' || char == '(' || char == ')')) {
            char.toString()
        } else {
            "%%%02X".format(code)
        }
    }

    internal fun escape(value: String): String = value
        .replace("\\", "\\\\")
        .replace("\r\n", "\\n")
        .replace("\n", "\\n")
        .replace("\r", "\\n")
        .replace(",", "\\,")
        .replace(";", "\\;")
}
