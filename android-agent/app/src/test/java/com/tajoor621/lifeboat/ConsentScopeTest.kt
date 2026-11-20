package com.tajoor621.lifeboat

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ConsentScopeTest {
    @Test fun freshInstallHasNoConsent() {
        assertFalse(SnapshotConsent(false, emptySet()).mayStartSnapshot())
    }

    @Test fun confirmationWithoutScopeIsRejected() {
        assertFalse(SnapshotConsent(true, emptySet()).mayStartSnapshot())
    }

    @Test fun explicitNarrowScopeIsAccepted() {
        assertTrue(SnapshotConsent(true, setOf(DataClass.CONTACTS)).mayStartSnapshot())
    }

    @Test fun selectingOneClassDoesNotImplyAnother() {
        val consent = SnapshotConsent(true, setOf(DataClass.PHOTOS))
        assertTrue(consent.mayStartSnapshot())
        assertFalse(DataClass.CONTACTS in consent.selectedClasses)
        assertFalse(DataClass.VIDEOS in consent.selectedClasses)
    }

    @Test fun unknownPermissionIsNeverGranted() {
        val observation = PermissionObservation(DataClass.CONTACTS, PermissionState.UNKNOWN)
        assertFalse(observation.hasAnyAccess())
        assertFalse(observation.hasFullAccess())
    }

    @Test fun limitedPermissionIsPartialNotFull() {
        val observation = PermissionObservation(DataClass.PHOTOS, PermissionState.LIMITED)
        assertTrue(observation.hasAnyAccess())
        assertFalse(observation.hasFullAccess())
    }

    @Test fun revokedPermissionHasNoAccess() {
        val observation = PermissionObservation(DataClass.CONTACTS, PermissionState.REVOKED)
        assertFalse(observation.hasAnyAccess())
    }
}
