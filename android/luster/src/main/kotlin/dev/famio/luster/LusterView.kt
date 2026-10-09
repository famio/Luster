package dev.famio.luster

import android.content.Context
import android.util.AttributeSet
import android.view.Choreographer
import android.view.MotionEvent
import android.view.TextureView
import android.view.VelocityTracker
import android.view.View
import android.view.animation.DecelerateInterpolator
import com.google.android.filament.android.UiHelper
import kotlin.math.pow

/** What the view is doing. */
sealed interface LusterState {
    object Idle : LusterState
    object Minting : LusterState
    data class Ready(val badge: LusterBadge) : LusterState
    data class Failed(val error: Throwable) : LusterState
}

/**
 * A badge struck from an SVG, lit in the studio. Drag to turn it; it keeps
 * spinning briefly with momentum.
 *
 * The view is clear round the badge: it draws the badge over whatever is
 * behind it, so a background is the layout's, under the view. It is a
 * [TextureView] for that, which composes with the views around it as a
 * window of its own could not.
 *
 * Minting runs off the main thread and the previous badge stays on screen while
 * a new one is struck. Setting [source] to a different document cancels the
 * one in flight; setting what is already there does nothing. For Compose,
 * `dev.famio.luster.compose.LusterView` draws the same badge without a View.
 */
class LusterView @JvmOverloads constructor(context: Context, attrs: AttributeSet? = null) :
    TextureView(context, attrs) {

    companion object {
        /** Where the badge sits before it is touched: slightly off-axis. */
        val restingPose: FloatArray get() = LusterStage.restingPose.copyOf()
    }

    private val stage = LusterStage(context)

    /** The document to strike. A different one mints a new badge. */
    var source: LusterSource?
        get() = stage.source
        set(value) {
            // The same source again does nothing, and leaves a placeholder
            // fading out to finish.
            if (value == stage.source) return
            stage.source = value
            updatePlaceholder(fade = false)
        }

    var options: LusterOptions
        get() = stage.options
        set(value) { stage.options = value }

    /** Lighting and metal. Changing it re-lights and re-plates; it never rebuilds. */
    var appearance: LusterAppearance
        get() = stage.appearance
        set(value) { stage.appearance = value }

    var onStateChange: ((LusterState) -> Unit)?
        get() = stage.onStateChange
        set(value) { stage.onStateChange = value }

    /**
     * Whether a flick keeps the badge turning. A flick never does while the
     * system has animations turned off, whatever this says.
     */
    var momentumEnabled: Boolean
        get() = stage.momentumEnabled
        set(value) { stage.momentumEnabled = value }

    /**
     * Shown while a source is set and no badge is on screen: while the first
     * is struck, and if it fails. Like a list's empty view, it is yours to lay
     * out, over this view; the view shows it, and fades it out as the badge
     * arrives. A badge struck later takes the place of the one on screen
     * without it.
     */
    var placeholderView: View? = null
        set(value) {
            if (value === field) return
            // The view shows only its own.
            field?.putAway()
            field = value
            updatePlaceholder(fade = false)
        }

    private val uiHelper = UiHelper(UiHelper.ContextErrorPolicy.DONT_CHECK)
    private val choreographer = Choreographer.getInstance()
    private var lastTouch = 0f to 0f
    private var tracker: VelocityTracker? = null

    init {
        uiHelper.isOpaque = false
        uiHelper.renderCallback = object : UiHelper.RendererCallback {
            override fun onNativeWindowChanged(surface: android.view.Surface) {
                stage.attach(surface, uiHelper.swapChainFlags, display)
            }

            override fun onDetachedFromSurface() = stage.detach()

            override fun onResized(width: Int, height: Int) = stage.resize(width, height)
        }
        uiHelper.attachTo(this)
        isClickable = true
        stage.onShowingChange = { updatePlaceholder(fade = true) }
    }

    private fun updatePlaceholder(fade: Boolean) {
        val placeholder = placeholderView ?: return
        if (source != null && !stage.showing) {
            placeholder.animate().cancel()
            placeholder.alpha = 1f
            placeholder.visibility = View.VISIBLE
        } else if (placeholder.visibility == View.VISIBLE) {
            if (!fade) {
                placeholder.putAway()
                return
            }
            placeholder.animate().alpha(0f).setDuration(200)
                .setInterpolator(DecelerateInterpolator())
                .withEndAction { placeholder.putAway() }
        }
    }

    /** Hidden, and whole for the next time it is shown. */
    private fun View.putAway() {
        animate().cancel()
        visibility = View.GONE
        alpha = 1f
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                // The drag is the badge's, in a scrolling list or a pager too.
                parent?.requestDisallowInterceptTouchEvent(true)
                stage.dragStarted()
                lastTouch = event.x to event.y
                tracker?.recycle()
                tracker = VelocityTracker.obtain().apply { addMovement(event) }
            }
            MotionEvent.ACTION_MOVE -> {
                tracker?.addMovement(event)
                val dx = event.x - lastTouch.first
                val dy = event.y - lastTouch.second
                lastTouch = event.x to event.y
                stage.dragged(dx * pointScale(), dy * pointScale())
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                if (event.actionMasked == MotionEvent.ACTION_UP) {
                    // The lift may come with the last of the movement: it counts too.
                    val dx = event.x - lastTouch.first
                    val dy = event.y - lastTouch.second
                    if (dx != 0f || dy != 0f) stage.dragged(dx * pointScale(), dy * pointScale())
                }
                val tracker = tracker
                this.tracker = null
                var velocity: Float? = null
                if (tracker != null) {
                    tracker.addMovement(event)
                    // Pixels a second, then points.
                    tracker.computeCurrentVelocity(1000)
                    if (event.actionMasked == MotionEvent.ACTION_UP) {
                        velocity = tracker.xVelocity * pointScale()
                    }
                    tracker.recycle()
                }
                stage.dragEnded(velocity)
            }
        }
        return true
    }

    /** Apple measures a drag in points; this turns Android's pixels into them. */
    private fun pointScale() = 160f / resources.displayMetrics.densityDpi

    private val frames = object : Choreographer.FrameCallback {
        override fun doFrame(frameTimeNanos: Long) {
            choreographer.postFrameCallback(this)
            stage.frame(frameTimeNanos, draw = uiHelper.isReadyToRender)
        }
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        if (!stage.released) choreographer.postFrameCallback(frames)
    }

    override fun onDetachedFromWindow() {
        super.onDetachedFromWindow()
        choreographer.removeFrameCallback(frames)
        stage.pause()
    }

    /** Frees the engine. Call it when the view is done with; the view does nothing after. */
    fun release() {
        if (stage.released) return
        choreographer.removeFrameCallback(frames)
        tracker?.recycle()
        tracker = null
        uiHelper.detach()
        stage.release()
    }
}

/** sRGB to linear, for the colours Filament takes. */
internal fun Float.linear(): Float =
    if (this <= 0.04045f) this / 12.92f else ((this + 0.055f) / 1.055f).pow(2.4f)
