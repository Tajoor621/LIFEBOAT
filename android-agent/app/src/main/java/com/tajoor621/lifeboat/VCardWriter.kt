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
        writer.append("BEGIN:VCARD\r\nVERSION:4.0\r\n")
        val name = contact.formattedName.ifBlank {
            listOf(contact.givenName, contact.familyName).filter { it.isNotBlank() }.joinToString(" ")
        }
        writer.append("FN:").append(escape(name.ifBlank { "Unnamed contact" })).append("\r\n")
        writer.append("N:").append(escape(contact.familyName)).append(";")
            .append(escape(contact.givenName)).append(";;;\r\n")
        contact.phones.filter(String::isNotBlank).distinct().forEach {
            writer.append("TEL;VALUE=text:").append(escape(it)).append("\r\n")
        }
        contact.emails.filter(String::isNotBlank).distinct().forEach {
            writer.append("EMAIL:").append(escape(it)).append("\r\n")
        }
        contact.organizations.filter(String::isNotBlank).distinct().forEach {
            writer.append("ORG:").append(escape(it)).append("\r\n")
        }
        contact.addresses.filter(String::isNotBlank).distinct().forEach {
            writer.append("ADR:;;").append(escape(it)).append(";;;;\r\n")
        }
        writer.append("END:VCARD\r\n")
        writer.flush()
    }

    internal fun escape(value: String): String = value
        .replace("\\", "\\\\")
        .replace("\r\n", "\\n")
        .replace("\n", "\\n")
        .replace("\r", "\\n")
        .replace(",", "\\,")
        .replace(";", "\\;")
}
