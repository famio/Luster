package dev.famio.luster

import android.content.Context
import android.graphics.Bitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

/**
 * Renders a badge to a still image, without a view.
 *
 * For thumbnails in a list, where a live view of each badge would cost far
 * more than the picture is worth. A still is drawn as [LusterView] draws, so
 * it is the picture a view shows. The first one sets up a renderer that is
 * kept for the process and draws one still at a time.
 */
object LusterSnapshot {

    /**
     * A square still of the badge [source] strikes, at [pose] (tilt, spin;
     * radians), [pixelSize] across, over [background], or with nothing
     * behind it when that is null: the bitmap's alpha is then the badge's
     * coverage.
     */
    suspend fun bitmap(
        context: Context,
        source: LusterSource,
        options: LusterOptions = LusterOptions(),
        appearance: LusterAppearance = LusterAppearance(),
        pose: FloatArray = LusterView.restingPose,
        pixelSize: Int = 768,
        background: LusterColor? = null,
    ): Bitmap = bitmap(context, Luster.mint(source, options), appearance, pose, pixelSize, background)

    /** The same, for a badge already struck. */
    suspend fun bitmap(
        context: Context,
        badge: LusterBadge,
        appearance: LusterAppearance = LusterAppearance(),
        pose: FloatArray = LusterView.restingPose,
        pixelSize: Int = 768,
        background: LusterColor? = null,
    ): Bitmap {
        require(pixelSize > 0) { "a still needs pixels: $pixelSize" }
        require(pose.size == 2) { "a pose is a tilt and a spin" }
        return turn.withLock {
            withContext(Dispatchers.Main.immediate) {
                val stage = stage ?: LusterStage(context.applicationContext).also { stage = it }
                stage.appearance = appearance
                stage.still(badge, pose, pixelSize, background)
            }
        }
    }

    /** The renderer stills are drawn with, made on the main thread with the first. */
    private var stage: LusterStage? = null
    private val turn = Mutex()
}
