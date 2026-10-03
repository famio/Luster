plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.plugin.compose")
    id("maven-publish")
}

group = "dev.famio"
version = "0.1.0"

android {
    namespace = "dev.famio.luster.compose"
    compileSdk = 36

    defaultConfig {
        minSdk = 24
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures { compose = true }
}

publishing {
    publications {
        register<MavenPublication>("release") {
            afterEvaluate { from(components["release"]) }
            artifactId = "luster-compose"
            pom {
                name = "Luster for Compose"
                description = "The Luster badge view as a composable, drawn with Filament."
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

dependencies {
    api(project(":luster"))
    val compose = platform("androidx.compose:compose-bom:2025.06.01")
    implementation(compose)
    // AndroidEmbeddedExternalSurface is foundation's.
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.ui:ui")
}
