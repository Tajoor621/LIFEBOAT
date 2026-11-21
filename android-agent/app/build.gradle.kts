plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    // Stable app identity: package ID follows the LIFEBOAT product name.
    namespace = "com.tajoor621.lifeboat"
    compileSdk = 35
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    defaultConfig {
        applicationId = "com.tajoor621.lifeboat"
        minSdk = 26
        targetSdk = 35
        versionCode = 2
        versionName = "0.2.2-p2-contacts"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
    buildTypes {
        // Keep the debug package ID identical to the app's stable application ID.
        // CI's default debug certificate is for testing only; production signing
        // must use a protected, persistent keystore supplied through CI secrets.
        debug {
            isDebuggable = true
        }
        release {
            isMinifyEnabled = false
        }
    }
    lint { abortOnError = true; checkReleaseBuilds = true }
}

dependencies {
    implementation("androidx.core:core-ktx:1.15.0")
    testImplementation("junit:junit:4.13.2")
}
