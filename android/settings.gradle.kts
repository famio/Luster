pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
    }
}

plugins {
    id("com.android.library") version "9.0.1" apply false
    id("com.android.application") version "9.0.1" apply false
    // The Compose compiler is Kotlin's own, so its version is the Kotlin AGP brings.
    id("org.jetbrains.kotlin.plugin.compose") version "2.2.10" apply false
    id("com.vanniktech.maven.publish") version "0.37.0" apply false
}

rootProject.name = "Luster"
include(":luster", ":luster-compose", ":sample")
