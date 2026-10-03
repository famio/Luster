package dev.famio.luster.sample

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import android.view.Choreographer
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import dev.famio.luster.LusterBadge
import dev.famio.luster.LusterColor
import dev.famio.luster.LusterSource
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * What the demo shows: the document, what the SVG button calls it, and the
 * menus' settings. The same whichever view draws the badge.
 */
class DemoState(source: LusterSource, title: String, settings: DemoSettings) {
    var source by mutableStateOf(source)
    var title by mutableStateOf(title)
    var settings by mutableStateOf(settings)

    companion object {
        /** The document the demo opens on; any other is opened from a file. */
        const val SAMPLE = "namiura"
        const val SAMPLE_TITLE = "Namiura"

        fun of(context: Context, launch: Launch) = when (val url = launch.url) {
            null -> DemoState(
                sample(context, launch.sample),
                if (launch.sample == SAMPLE) SAMPLE_TITLE else launch.sample,
                launch.settings,
            )
            else -> DemoState(
                LusterSource.url(url),
                url.substringAfterLast('/').replace(Regex("\\.svg$", RegexOption.IGNORE_CASE), ""),
                launch.settings,
            )
        }
    }
}

/** A bundled SVG, by its asset name. */
fun sample(context: Context, name: String): LusterSource =
    LusterSource.bytes(context.assets.open("$name.svg").use { it.readBytes() })

/** Reads an SVG the user picked, with the name to call it by. */
suspend fun readSvg(context: Context, uri: Uri): Pair<ByteArray, String> = withContext(Dispatchers.IO) {
    val bytes = context.contentResolver.openInputStream(uri)?.use { it.readBytes() }
        ?: error("cannot open $uri")
    val name = context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
        ?.use { if (it.moveToFirst()) it.getString(0) else null }
    bytes to (name?.replace(Regex("\\.svg$", RegexOption.IGNORE_CASE), "") ?: "SVG")
}

/** Writes [badge] as a GLB, in [metal], to [uri]; returns its size in bytes. */
suspend fun writeGlb(context: Context, uri: Uri, badge: LusterBadge, metal: LusterColor): Int {
    val glb = badge.glb(metal = metal)
    withContext(Dispatchers.IO) {
        context.contentResolver.openOutputStream(uri)?.use { it.write(glb) } ?: error("cannot open $uri")
    }
    return glb.size
}

/**
 * The longest time between frames since [reset], in milliseconds. A mint
 * that blocked the main thread shows up as a long gap.
 */
class FrameGaps {
    var longest = 0L
        private set
    private var last = 0L
    private val choreographer = Choreographer.getInstance()
    private val frames = object : Choreographer.FrameCallback {
        override fun doFrame(now: Long) {
            if (last > 0) longest = maxOf(longest, (now - last) / 1_000_000)
            last = now
            choreographer.postFrameCallback(this)
        }
    }

    fun start() = choreographer.postFrameCallback(frames)

    fun stop() {
        choreographer.removeFrameCallback(frames)
        last = 0L
    }

    fun reset() {
        longest = 0L
    }
}
