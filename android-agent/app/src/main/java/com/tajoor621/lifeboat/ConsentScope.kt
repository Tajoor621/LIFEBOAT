package com.tajoor621.lifeboat

/**
 * Data classes that may be separately enabled by the device owner.
 * This model does not itself request permissions or begin collection.
 */
enum class DataClass {
    CONTACTS,
    PHOTOS,
    VIDEOS,
    DOCUMENTS
}

data class SnapshotConsent(
    val explicitlyConfirmed: Boolean,
    val selectedClasses: Set<DataClass>
) {
    fun mayStartSnapshot(): Boolean =
        explicitlyConfirmed && selectedClasses.isNotEmpty()
}
