package com.tajoor621.lifeboat

enum class DataClass { CONTACTS, PHOTOS, VIDEOS, DOCUMENTS }

data class SnapshotConsent(
    val explicitlyConfirmed: Boolean,
    val selectedClasses: Set<DataClass>
) {
    fun mayStartSnapshot(): Boolean =
        explicitlyConfirmed && selectedClasses.isNotEmpty()
}

enum class PermissionState {
    NOT_REQUESTED, GRANTED, DENIED, PERMANENTLY_DENIED,
    LIMITED, REVOKED, NOT_APPLICABLE, UNKNOWN
}

data class PermissionObservation(
    val dataClass: DataClass,
    val state: PermissionState
) {
    fun hasAnyAccess(): Boolean =
        state == PermissionState.GRANTED || state == PermissionState.LIMITED

    fun hasFullAccess(): Boolean = state == PermissionState.GRANTED
}
