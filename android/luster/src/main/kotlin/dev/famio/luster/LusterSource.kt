package dev.famio.luster

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.runInterruptible
import kotlinx.coroutines.withContext
import java.io.ByteArrayOutputStream
import java.io.InputStream
import java.net.HttpURLConnection
import java.net.URL
import kotlin.coroutines.coroutineContext

/**
 * Where a badge's SVG comes from: its bytes, its text, or a web URL fetched
 * with [Luster.loader].
 */
class LusterSource private constructor(
    private val bytes: ByteArray?,
    /** The http(s) URL to fetch, for a web source. */
    val url: String?,
) {
    companion object {
        /** SVG document bytes. */
        fun bytes(bytes: ByteArray) = LusterSource(bytes, null)

        /** SVG document text. */
        fun svg(text: String) = LusterSource(text.toByteArray(), null)

        /** An http(s) URL, fetched with [Luster.loader]. */
        fun url(url: String): LusterSource {
            require(url.startsWith("https://", ignoreCase = true) || url.startsWith("http://", ignoreCase = true)) {
                "a web source is http(s): $url"
            }
            return LusterSource(null, url)
        }

        /** Larger documents are refused before they are read in full. */
        const val MAXIMUM_BYTES = 20 shl 20
    }

    /** The SVG bytes, fetched off the main thread for a web source. */
    internal suspend fun load(): ByteArray {
        bytes?.let {
            if (it.size > MAXIMUM_BYTES) throw LusterException.TooLarge(it.size.toLong())
            return it
        }
        val data = try {
            Luster.loader.load(url!!)
        } catch (e: LusterException) {
            throw e
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            throw LusterException.UnreadableSource("could not fetch $url: ${e.message}", e)
        }
        if (data.size > MAXIMUM_BYTES) throw LusterException.TooLarge(data.size.toLong())
        return data
    }

    override fun equals(other: Any?) = other is LusterSource &&
        url == other.url && (bytes === other.bytes || bytes != null && other.bytes != null && bytes.contentEquals(other.bytes))

    override fun hashCode() = url?.hashCode() ?: bytes.contentHashCode()

    override fun toString() = url?.let { "LusterSource($it)" } ?: "LusterSource(${bytes!!.size} bytes)"
}

/**
 * Fetches the document a web [LusterSource] names. Set one as [Luster.loader]
 * to fetch through a client and cache the app already has, such as OkHttp's
 * (see the README); the default is [HttpURLConnection], which keeps nothing
 * unless the app has installed an `HttpResponseCache`.
 */
fun interface LusterLoader {
    suspend fun load(url: String): ByteArray
}

/** Fetches with [HttpURLConnection], on [Dispatchers.IO]. */
object LusterHttpLoader : LusterLoader {
    override suspend fun load(url: String): ByteArray = withContext(Dispatchers.IO) {
        val connection = URL(url).openConnection() as HttpURLConnection
        connection.connectTimeout = 15_000
        connection.readTimeout = 30_000
        try {
            val status = runInterruptible { connection.responseCode }
            if (status !in 200..299) throw LusterException.UnreadableSource("could not fetch $url: HTTP $status")
            val length = connection.contentLengthLong
            if (length > LusterSource.MAXIMUM_BYTES) throw LusterException.TooLarge(length)
            connection.inputStream.use { read(it) }
        } finally {
            connection.disconnect()
        }
    }

    /** Reads to the end, giving up past the limit or once cancelled. */
    private suspend fun read(stream: InputStream): ByteArray {
        val out = ByteArrayOutputStream()
        val buffer = ByteArray(64 shl 10)
        while (true) {
            coroutineContext.ensureActive()
            val n = runInterruptible { stream.read(buffer) }
            if (n < 0) return out.toByteArray()
            out.write(buffer, 0, n)
            if (out.size() > LusterSource.MAXIMUM_BYTES) throw LusterException.TooLarge(out.size().toLong())
        }
    }
}
