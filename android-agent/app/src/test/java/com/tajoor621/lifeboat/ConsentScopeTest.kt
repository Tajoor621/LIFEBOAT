package com.tajoor621.lifeboat

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ConsentScopeTest {
    @Test
    fun freshInstallHasNoConsent() {
        assertFalse(SnapshotConsent(false, emptySet()).mayStartSnapshot())
    }

    @Test
    fun confirmationWithoutSelectedDataClassIsNotConsentToCollect() {
        assertFalse(SnapshotConsent(true, emptySet()).mayStartSnapshot())
    }

    @Test
    fun explicitConfirmationAndNarrowScopeAllowsSnapshot() {
        assertTrue(SnapshotConsent(true, setOf(DataClass.CONTACTS)).mayStartSnapshot())
    }

    @Test
    fun selectingOneClassDoesNotImplyAnother() {
        val consent = SnapshotConsent(true, setOf(DataClass.PHOTOS))
        assertTrue(consent.mayStartSnapshot())
        assertFalse(DataClass.CONTACTS in consent.selectedClasses)
        assertFalse(DataClass.VIDEOS in consent.selectedClasses)
    }
}
