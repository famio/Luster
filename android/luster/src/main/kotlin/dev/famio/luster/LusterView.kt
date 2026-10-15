package dev.famio.luster

import android.annotation.SuppressLint
import android.content.Context
import android.util.AttributeSet
import android.view.Choreographer
import android.view.MotionEvent
import android.view.TextureView
import android.view.VelocityTracker
import android.view.View
import android.view.ViewConfiguration
import android.view.animation.DecelerateInterpolator
import com.google.android.filament.android.UiHelper
import kotlin.math.hypot
import kotlin.math.max
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
 * spinning briefly with momentum. Pinch to bring it closer and slide two
 * fingers to look about it; a double tap puts it back.
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

    /** Whether a pinch brings the badge closer. Turned off, the badge is put back at its size. */
    var zoomEnabled: Boolean
        get() = stage.zoomEnabled
        set(value) { stage.zoomEnabled = value }

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
    private val touchConfig = ViewConfiguration.get(context)
    /** Where the finger came down, and when. */
    private var touchDown = Triple(0f, 0f, 0L)
    /** The last tap, for a second one to make a double tap. */
    private var lastTap: Triple<Float, Float, Long>? = null
    /** The fingers' spread as a pinch began, while one is going on. */
    private var pinchSpread: Float? = null
    /** A pinch was made since the fingers came down: the one left behind does not turn the badge. */
    private var pinched = false

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
        stage.onFrameWanted = { schedule() }
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

    // A touch turns the badge; there is no click for performClick to stand for.
    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                // The drag is the badge's, in a scrolling list or a pager too.
                parent?.requestDisallowInterceptTouchEvent(true)
                stage.dragStarted()
                lastTouch = event.x to event.y
                touchDown = Triple(event.x, event.y, event.eventTime)
                pinched = false
                tracker?.recycle()
                tracker = VelocityTracker.obtain().apply { addMovement(event) }
            }
            MotionEvent.ACTION_POINTER_DOWN -> {
                if (event.pointerCount == 2 && zoomEnabled && pinchSpread == null) {
                    // The drag gives way to the pinch, without a flick.
                    pinched = true
                    stage.dragEnded(null)
                    pinchSpread = spread(event)
                    stage.pinchStarted(middle(event).first, middle(event).second)
                }
            }
            MotionEvent.ACTION_POINTER_UP -> {
                if (pinchSpread != null && event.pointerCount <= 2) {
                    pinchSpread = null
                    stage.pinchEnded()
                }
            }
            MotionEvent.ACTION_MOVE -> {
                val spread = pinchSpread
                if (spread != null) {
                    val (x, y) = middle(event)
                    stage.pinched(spread(event) / spread, x, y)
                    return true
                }
                if (pinched) return true
                tracker?.addMovement(event)
                val dx = event.x - lastTouch.first
                val dy = event.y - lastTouch.second
                lastTouch = event.x to event.y
                stage.dragged(dx * pointScale(), dy * pointScale())
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                if (pinched) {
                    pinchSpread = null
                    pinched = false
                    stage.pinchEnded()
                    tracker?.recycle()
                    tracker = null
                    return true
                }
                if (event.actionMasked == MotionEvent.ACTION_UP) {
                    tapped(event)
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

    /** The distance between the first two fingers, pixels. */
    private fun spread(event: MotionEvent) =
        max(1f, hypot(event.getX(0) - event.getX(1), event.getY(0) - event.getY(1)))

    /** The point halfway between the first two fingers. */
    private fun middle(event: MotionEvent) =
        (event.getX(0) + event.getX(1)) / 2 to (event.getY(0) + event.getY(1)) / 2

    /** A finger lifted where it came down, soon: a second one soon after puts the badge back. */
    private fun tapped(event: MotionEvent) {
        val (x, y, time) = touchDown
        val slop = touchConfig.scaledDoubleTapSlop.toFloat()
        val timeout = ViewConfiguration.getDoubleTapTimeout()
        if (event.eventTime - time > timeout || hypot(event.x - x, event.y - y) > touchConfig.scaledTouchSlop) return
        val last = lastTap
        if (last != null && time - last.third <= timeout && hypot(x - last.first, y - last.second) <= slop) {
            lastTap = null
            stage.putBack()
        } else {
            lastTap = Triple(x, y, event.eventTime)
        }
    }

    /** Apple measures a drag in points; this turns Android's pixels into them. */
    private fun pointScale() = 160f / resources.displayMetrics.densityDpi

    /** Whether a frame callback is posted. Frames are asked for only while the stage wants them. */
    private var scheduled = false

    private val frames = object : Choreographer.FrameCallback {
        override fun doFrame(frameTimeNanos: Long) {
            scheduled = false
            stage.frame(frameTimeNanos, draw = uiHelper.isReadyToRender)
            if (stage.wantsFrames) schedule()
        }
    }

    private fun schedule() {
        if (scheduled || !isAttachedToWindow || stage.released) return
        scheduled = true
        choreographer.postFrameCallback(frames)
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        schedule()
    }

    override fun onDetachedFromWindow() {
        super.onDetachedFromWindow()
        choreographer.removeFrameCallback(frames)
        scheduled = false
        stage.pause()
    }

    /** Frees the engine. Call it when the view is done with; the view does nothing after. */
    fun release() {
        if (stage.released) return
        choreographer.removeFrameCallback(frames)
        scheduled = false
        tracker?.recycle()
        tracker = null
        uiHelper.detach()
        stage.release()
    }
}

/** sRGB to linear, for the colours Filament takes. */
internal fun Float.linear(): Float =
    if (this <= 0.04045f) this / 12.92f else ((this + 0.055f) / 1.055f).pow(2.4f)
