package dev.famio.luster

/**
 * Why a badge could not be made: the document, its source, or the engine.
 * Giving a mint up is not one of these; it ends in a `CancellationException`,
 * as any cancelled coroutine does.
 */
sealed class LusterException(message: String, cause: Throwable? = null) : Exception(message, cause) {
    /** The data is not an SVG document. */
    class InvalidSvg(val reason: String) : LusterException("the SVG could not be read: $reason")

    /** The SVG exceeds the engine's limits (shapes, path segments, …). */
    class InputTooComplex(val reason: String) : LusterException("the SVG is too complex: $reason")

    /** The SVG draws nothing a badge can be made of. */
    class NothingToMint : LusterException("the SVG has nothing to mint")

    /** The source is larger than [LusterSource.MAXIMUM_BYTES]. */
    class TooLarge(val size: Long) :
        LusterException("the document is $size bytes; at most ${LusterSource.MAXIMUM_BYTES} are read")

    /** The URL could not be fetched; [reason] says why, and [cause] is what the loader threw. */
    class UnreadableSource(val reason: String, cause: Throwable? = null) : LusterException(reason, cause)

    /** The engine itself went wrong: a bug, not the document's doing. */
    class EngineFailure(val reason: String) : LusterException("the engine failed: $reason")
}
