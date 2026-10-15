plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "dev.famio.luster.sample"
    compileSdk = 36

    defaultConfig {
        applicationId = "dev.famio.luster.sample"
        minSdk = 24
        targetSdk = 36
        versionCode = 1
        versionName = "0.4.0"
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    // The same documents the engine is tested on.
    sourceSets["main"].assets.directories.add("../../fixtures/svg")
    buildTypes {
        release { isMinifyEnabled = false }
    }
    buildFeatures { compose = true }
    lint {
        warningsAsErrors = true
        // A demo, started from Android Studio or adb, as the iOS one is from Xcode.
        disable += "MissingApplicationIcon"
    }
}

dependencies {
    implementation(project(":luster"))
    implementation(project(":luster-compose"))
    implementation(platform("androidx.compose:compose-bom:2025.06.01"))
    implementation("androidx.compose.material3:material3")
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.2")
}
