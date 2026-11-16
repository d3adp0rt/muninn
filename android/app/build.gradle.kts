plugins {
    id("com.android.application")
}

android {
    namespace = "io.github.d3adp0rt.muninn"
    compileSdk = 36

    defaultConfig {
        applicationId = "io.github.d3adp0rt.muninn"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"

        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }

    buildFeatures {
        buildConfig = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    packaging {
        // Required to exec the bundled CPython binary from nativeLibraryDir.
        jniLibs {
            useLegacyPackaging = true
        }
    }

    buildTypes {
        debug {
            isMinifyEnabled = false
        }
        release {
            isMinifyEnabled = false
            isShrinkResources = false
        }
    }
}

dependencies {
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.core:core:1.17.0")

    // GameActivity: IME support + AppCompat base.
    implementation("androidx.games:games-activity:4.4.0")
}
