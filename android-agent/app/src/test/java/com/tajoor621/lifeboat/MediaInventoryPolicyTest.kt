package com.tajoor621.lifeboat

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class MediaInventoryPolicyTest {
    @Test fun selectedCategoriesRemainIndependent() {
        val selected = setOf(DataClass.PHOTOS)
        assertTrue(DataClass.PHOTOS in selected)
        assertFalse(DataClass.VIDEOS in selected)
    }
    @Test fun limitedScopeIsNotFull() {
        val permission = PermissionObservation(DataClass.PHOTOS, PermissionState.LIMITED)
        assertTrue(permission.hasAnyAccess())
        assertFalse(permission.hasFullAccess())
    }
    @Test fun revokedMediaPermissionDoesNotGrantAccess() {
        assertFalse(PermissionObservation(DataClass.VIDEOS, PermissionState.REVOKED).hasAnyAccess())
    }
}
