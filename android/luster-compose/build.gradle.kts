import com.vanniktech.maven.publish.AndroidSingleVariantLibrary
import com.vanniktech.maven.publish.JavadocJar
import com.vanniktech.maven.publish.SourcesJar

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.plugin.compose")
    id("com.vanniktech.maven.publish")
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

mavenPublishing {
    // AGP's own javadoc tool cannot read the engine's sealed classes; an IDE
    // shows the docs from the sources jar, and Central takes an empty javadoc jar.
    configure(AndroidSingleVariantLibrary(JavadocJar.Empty(), SourcesJar.Sources()))
    publishToMavenCentral()
    // Central takes only signed files; a local build has no key and needs none.
    if (providers.gradleProperty("signingInMemoryKey").isPresent) signAllPublications()
    pom {
        name = "Luster for Compose"
        description = "The Luster badge view as a composable, drawn with Filament."
        inceptionYear = "2026"
        url = "https://github.com/famio/Luster"
        licenses {
            license {
                name = "MIT License"
                url = "https://github.com/famio/Luster/blob/main/LICENSE"
                distribution = "repo"
            }
        }
        developers {
            developer {
                id = "famio"
                name = "famio"
                url = "https://github.com/famio"
            }
        }
        scm {
            url = "https://github.com/famio/Luster"
            connection = "scm:git:https://github.com/famio/Luster.git"
            developerConnection = "scm:git:ssh://git@github.com/famio/Luster.git"
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
