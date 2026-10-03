package dev.famio.luster.sample

import android.os.Bundle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.appcompat.app.AppCompatActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import dev.famio.luster.LusterView as LusterViewClass
import dev.famio.luster.compose.LusterView

/**
 * The Android demo, the same as the iOS and Flutter ones (see [Demo]): it
 * opens on Namiura, and its tabs draw the badge with the Compose view or the
 * View one. It follows the system's light or dark appearance. See [Launch]
 * for what adb can set.
 */
class MainActivity : AppCompatActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val launch = Launch.of(intent)
        setContent {
            val dark = isSystemInDarkTheme()
            MaterialTheme(colorScheme = if (dark) darkColorScheme() else lightColorScheme()) {
                val background = Modifier.fillMaxSize().background(if (dark) Color.Black else Color.White)
                when {
                    launch.bare -> Bare(launch, background)
                    launch.list -> BadgeList(launch.settings, launch.stills, background)
                    else -> {
                        val context = LocalContext.current
                        val state = remember { DemoState.of(context, launch) }
                        Demo(state, dark, launch.tab, Modifier.fillMaxSize())
                    }
                }
            }
        }
    }

    /** One tab's badge alone, filling the window: a still to compare with the other's. */
    @Composable
    private fun Bare(launch: Launch, modifier: Modifier) {
        val context = LocalContext.current
        val svg = remember { sample(context, launch.sample) }
        Box(modifier) {
            when (launch.tab) {
                Launch.Tab.COMPOSE -> LusterView(
                    source = svg,
                    modifier = Modifier.fillMaxSize(),
                    options = launch.settings.options,
                    appearance = launch.settings.appearance,
                )
                Launch.Tab.VIEW -> AndroidView(
                    factory = {
                        LusterViewClass(it).apply {
                            options = launch.settings.options
                            appearance = launch.settings.appearance
                            source = svg
                        }
                    },
                    modifier = Modifier.fillMaxSize(),
                    onRelease = { it.release() },
                )
            }
        }
    }
}
