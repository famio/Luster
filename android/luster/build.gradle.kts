plugins {
    // AGP 9 brings Kotlin support of its own; the Kotlin plugin is not applied.
    id("com.android.library")
    id("maven-publish")
}

group = "dev.famio"
version = "0.1.0"

android {
    namespace = "dev.famio.luster"
    compileSdk = 36

    defaultConfig {
        minSdk = 24
        // The .so files Scripts/build-android.sh leaves here.
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    // 16 KB pages: Android 15 devices may use them, and a library that is not
    // aligned will not load there.
    packaging { jniLibs { useLegacyPackaging = false } }
}

publishing {
    publications {
        register<MavenPublication>("release") {
            afterEvaluate { from(components["release"]) }
            artifactId = "luster"
            pom {
                name = "Luster"
                description = "Strikes an SVG into a 3D gold enamel pin, drawn with Filament."
                url = "https://github.com/famio/Luster"
                licenses {
                    license {
                        name = "MIT"
                        url = "https://github.com/famio/Luster/blob/main/LICENSE"
                    }
                }
            }
        }
    }
    repositories {
        maven {
            name = "GitHubPackages"
            url = uri("https://maven.pkg.github.com/famio/Luster")
            credentials {
                username = System.getenv("GITHUB_ACTOR")
                password = System.getenv("GITHUB_TOKEN")
            }
        }
    }
}

android {
    publishing {
        singleVariant("release") {
            withSourcesJar()
        }
    }
}

// The engine is built outside Gradle, by Scripts/build-android.sh. Without it
// an AAR still assembles, and loads nothing: packing the native libraries
// checks they are there first.
val engineBuilt by tasks.registering {
    val libs = layout.projectDirectory.dir("src/main/jniLibs")
    doLast {
        for (abi in listOf("arm64-v8a", "x86_64")) {
            check(libs.file("$abi/libluster_ffi.so").asFile.exists()) {
                "no engine for $abi in src/main/jniLibs: run Scripts/build-android.sh first"
            }
        }
    }
}
tasks.configureEach {
    if (name.startsWith("merge") && name.endsWith("JniLibFolders")) dependsOn(engineBuilt)
}

dependencies {
    // uniffi's Kotlin bindings call the engine through JNA.
    implementation("net.java.dev.jna:jna:5.18.1@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.2")
    // @RestrictTo: the stage is public for luster-compose, not for apps.
    implementation("androidx.annotation:annotation:1.9.1")
    // Drawing: Filament, and gltfio for the ubershaders a badge is shaded with.
    api("com.google.android.filament:filament-android:1.70.0")
    api("com.google.android.filament:gltfio-android:1.70.0")
    api("com.google.android.filament:filament-utils-android:1.70.0")

    testImplementation("junit:junit:4.13.2")
}
