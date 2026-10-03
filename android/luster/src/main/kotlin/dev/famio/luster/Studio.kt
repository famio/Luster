package dev.famio.luster

import dev.famio.luster.core.showcaseCubemap
import dev.famio.luster.core.showcaseIrradiance
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.FloatBuffer

/**
 * What every stage is lit with, made once for the process off the main
 * thread: the showcase's panorama as Filament's cubemaps take it, and the
 * reverse's grit. Each stage uploads them to its own Filament engine.
 */
internal object Studio {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val panoramaKept = Kept { Faces.of(showcaseCubemap(256u)) to Faces.of(showcaseIrradiance(16u)) }
    private val gritKept = Kept { Pixels.grit() }

    /** The reflections' faces, and the irradiance's. */
    val panorama: Deferred<Pair<Faces, Faces>> get() = panoramaKept.get(scope)

    /** The reverse's sandblast: the same for every badge. */
    val grit: Deferred<Pixels> get() = gritKept.get(scope)

    /** Starts making both, if nothing has yet. */
    fun prewarm() {
        panorama
        grit
    }
}

/** Made once and kept, unless making it failed: then the next ask tries again. */
private class Kept<T>(private val make: () -> T) {
    private var made: Deferred<T>? = null

    @Synchronized
    fun get(scope: CoroutineScope): Deferred<T> = made ?: scope.async {
        try {
            make()
        } catch (e: Throwable) {
            forget()
            throw e
        }
    }.also { made = it }

    @Synchronized
    private fun forget() {
        made = null
    }
}

/** A cubemap's six faces as linear floats, ready to upload. */
internal class Faces(val size: Int, val floats: FloatBuffer) {
    companion object {
        /** Converts the engine's stacked sRGB faces. Blocks: call it off the main thread. */
        fun of(texture: dev.famio.luster.core.Texture): Faces {
            val size = texture.width.toInt()
            // Three channels, linear, floating point: what Filament's
            // prefilter reads and, more to the point, what it writes back.
            val floats = ByteBuffer.allocateDirect(size * size * 6 * 3 * 4)
                .order(ByteOrder.nativeOrder())
                .asFloatBuffer()
            var at = 0
            while (at < texture.rgba.size) {
                for (channel in 0 until 3) {
                    floats.put(((texture.rgba[at + channel].toInt() and 0xFF) / 255f).linear())
                }
                at += 4
            }
            floats.rewind()
            return Faces(size, floats)
        }
    }
}
