plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.tajoor621.lifeboat"
    compileSdk = 35
    defaultConfig {
        applicationId = "com.tajoor621.lifeboat"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.2.2-p2-contacts"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
    buildTypes {
        debug { applicationIdSuffix = ".debug"; versionNameSuffix = "-debug" }
        release { isMinifyEnabled = false }
    }
    lint { abortOnError = true; checkReleaseBuilds = true }
}

dependencies {
    implementation("androidx.core:core-ktx:1.15.0")
    testImplementation("junit:junit:4.13.2")
}
