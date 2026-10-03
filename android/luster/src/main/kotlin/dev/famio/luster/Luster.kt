package dev.famio.luster

import dev.famio.luster.core.Lighting
import dev.famio.luster.core.MintOptions
import dev.famio.luster.core.MintRequest
import dev.famio.luster.core.Rgba
import dev.famio.luster.core.copper
import dev.famio.luster.core.defaultGold
import dev.famio.luster.core.silver
import dev.famio.luster.core.version
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** The metal's colour, sRGB 0…1. */
data class LusterColor(val red: Float, val green: Float, val blue: Float) {
    companion object {
        /** The engine's default metal: pale gold. */
        val Gold = defaultGold().let { LusterColor(it.r, it.g, it.b) }
        val Silver = silver().let { LusterColor(it.r, it.g, it.b) }
        val Copper = copper().let { LusterColor(it.r, it.g, it.b) }
    }

    internal fun core() = Rgba(red, green, blue, 1f)
}

/**
 * What to strike. Changing any of it rebuilds the badge.
 *
 * A badge is the art's silhouette in metal, its fills set into it as enamel
 * that runs on to the edge, recessed below the metal its lines stand in. The
 * metal's colour is not here: it changes no part of the badge, and belongs to
 * [LusterAppearance].
 */
data class LusterOptions(
    /**
     * Leave the lines and the edge's top in the metal. By default they are
     * plated in the colours the document paints on them, each line in its
     * stroke colour.
     */
    val metalLines: Boolean = false,
    /**
     * Leave out the faces no view can see. Worth it for a badge being sent
     * somewhere, not for one on screen: the search costs more than the
     * triangles do.
     */
    val withoutHiddenFaces: Boolean = false,
)

enum class LusterLighting {
    /**
     * A product photographer's studio: strip lights that sweep across the
     * enamel as a band of sheen when the badge turns, over a dim tent that
     * keeps plated lines their colour at any angle, and no lamps. The enamel
     * shows the colours the document paints.
     */
    SHOWCASE,
    /** For checking a badge's shape: lamps alone, no reflections, and nothing that shines. */
    OFF,
}

/**
 * How the badge is lit and plated. Changing it re-lights and re-plates; it never rebuilds.
 */
data class LusterAppearance(
    val metal: LusterColor = LusterColor.Gold,
    val lighting: LusterLighting = LusterLighting.SHOWCASE,
)

/** Entry points to the engine. */
object Luster {

    /** The engine's version. */
    val version: String get() = version()

    /**
     * What web sources are fetched with, process-wide: [LusterHttpLoader]
     * unless the app sets another.
     */
    @Volatile
    var loader: LusterLoader = LusterHttpLoader

    /**
     * Mints a badge from an SVG, on the engine's own threads; a web source is
     * fetched with [loader] first.
     *
     * Cancelling the calling coroutine stops the engine at its next
     * checkpoint, unless someone else still wants the same badge, and the call
     * ends with [CancellationException]. At most two mints run at once across
     * the process; the rest wait their turn. Recent badges are kept by their
     * bytes and options, so asking again is immediate, and asking for one
     * already being minted waits for that mint rather than starting another.
     *
     * @throws LusterException when the document, its source or the engine fails.
     */
    suspend fun mint(source: LusterSource, options: LusterOptions = LusterOptions()): LusterBadge {
        val svg = source.load()
        // Closing the request gives it up, if it has not come by then.
        return MintRequest(svg, MintOptions(options.withoutHiddenFaces, options.metalLines)).use { request ->
            try {
                LusterBadge(request.badge())
            } catch (e: dev.famio.luster.core.LusterException) {
                throw e.translated()
            }
        }
    }

    /**
     * Starts making what every view and still is lit with (the studio's
     * panorama and the reverse's grit) and loads the engine, so the first
     * badge on screen need not wait for them. Call it early, e.g. at launch.
     */
    fun prewarm() {
        Studio.prewarm()
    }
}

/** The engine's error as this side tells it; a mint given up is a cancellation. */
private fun dev.famio.luster.core.LusterException.translated(): Exception = when (this) {
    is dev.famio.luster.core.LusterException.InvalidSvg -> LusterException.InvalidSvg(reason)
    is dev.famio.luster.core.LusterException.InputTooComplex -> LusterException.InputTooComplex(reason)
    is dev.famio.luster.core.LusterException.NothingToMint -> LusterException.NothingToMint()
    is dev.famio.luster.core.LusterException.Internal -> LusterException.EngineFailure(reason)
    is dev.famio.luster.core.LusterException.Cancelled -> CancellationException("the mint was given up")
}

/**
 * A struck badge: what a view shows, and the bytes to send one somewhere else.
 * The views hand one over in [LusterState.Ready].
 */
class LusterBadge internal constructor(internal val core: dev.famio.luster.core.LusterBadge) {
    /**
     * Identifies the badge: the document and the options it was struck with.
     * Equal keys mean the same badge.
     */
    val designKey: String = core.designKey()

    /**
     * The badge as a glTF binary: meshes, materials and textures in one file,
     * its metal plated in [metal], [scale] badge-widths across. The same badge
     * always writes the same bytes. Runs on [Dispatchers.Default].
     */
    suspend fun glb(scale: Float = 1f, metal: LusterColor = LusterColor.Gold): ByteArray =
        withContext(Dispatchers.Default) { core.glb(scale, metal.core()) }

    override fun equals(other: Any?) = other is LusterBadge && other.designKey == designKey
    override fun hashCode() = designKey.hashCode()
    override fun toString() = "LusterBadge($designKey)"
}

internal fun LusterLighting.core() = when (this) {
    LusterLighting.SHOWCASE -> Lighting.SHOWCASE
    LusterLighting.OFF -> Lighting.OFF
}
