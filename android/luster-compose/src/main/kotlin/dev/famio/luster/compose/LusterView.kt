package dev.famio.luster.compose

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.AndroidEmbeddedExternalSurface
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.PointerId
import androidx.compose.ui.input.pointer.PointerInputChange
import androidx.compose.ui.input.pointer.changedToUpIgnoreConsumed
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import android.view.ViewConfiguration
import dev.famio.luster.LusterAppearance
import dev.famio.luster.LusterOptions
import dev.famio.luster.LusterSource
import dev.famio.luster.LusterStage
import dev.famio.luster.LusterState
import com.google.android.filament.SwapChainFlags
import kotlinx.coroutines.CancellationException

/**
 * A badge struck from an SVG, lit in the studio, as a composable: the same
 * badge `dev.famio.luster.LusterView` draws, from the same stage, without a View.
 * Drag to turn it; it keeps spinning briefly with momentum. Pinch to bring it
 * closer and slide two fingers to look about it; a double tap puts it back.
 *
 * It is clear round the badge and drawn over whatever is behind it, so a
 * background is the layout's. The drag is the badge's, inside a scrolling
 * list or a pager too. The engine is freed when it leaves the composition.
 *
 * Minting runs off the main thread and the previous badge stays on screen
 * while a new one is struck. A different [source] or [options] mints again,
 * the same ones do nothing; a different [appearance] re-lights and re-plates.
 *
 * Until a badge is on screen it shows its placeholder, if it is given one:
 *
 * ```kotlin
 * LusterView(source, Modifier.size(280.dp), placeholder = { state ->
 *     if (state is LusterState.Failed) Text("Could not make the badge")
 *     else CircularProgressIndicator()
 * })
 * ```
 */
@Composable
fun LusterView(
    source: LusterSource?,
    modifier: Modifier = Modifier,
    options: LusterOptions = LusterOptions(),
    appearance: LusterAppearance = LusterAppearance(),
    /** Whether a flick keeps the badge turning; never while the system has animations off. */
    momentumEnabled: Boolean = true,
    /** Whether a pinch brings the badge closer. Turned off, the badge is put back at its size. */
    zoomEnabled: Boolean = true,
    /**
     * Shown over the view, in the middle, while a source is set and no badge
     * is on screen: while the first is struck, given [LusterState.Minting],
     * and if it fails, given [LusterState.Failed]. A badge struck later takes
     * the place of the one on screen without it.
     */
    placeholder: (@Composable (LusterState) -> Unit)? = null,
    onStateChange: ((LusterState) -> Unit)? = null,
) {
    val context = LocalContext.current
    val display = LocalView.current.display
    val stage = remember { LusterStage(context) }
    DisposableEffect(stage) {
        onDispose { stage.release() }
    }
    val currentOnState by rememberUpdatedState(onStateChange)
    var showing by remember { mutableStateOf(false) }
    var failed by remember { mutableStateOf<LusterState.Failed?>(null) }
    SideEffect {
        stage.onShowingChange = { showing = it }
        stage.onStateChange = {
            failed = it as? LusterState.Failed
            currentOnState?.invoke(it)
        }
        stage.momentumEnabled = momentumEnabled
        stage.zoomEnabled = zoomEnabled
        // The options before the document, so a first mint is struck with them.
        stage.options = options
        stage.appearance = appearance
        stage.source = source
    }
    LaunchedEffect(stage) {
        // Frames only while the stage wants them: a badge at rest stays as
        // last drawn.
        while (true) {
            stage.frameWanted()
            withFrameNanos { stage.frame(it) }
        }
    }

    Box(modifier) {
        AndroidEmbeddedExternalSurface(
            modifier = Modifier.fillMaxSize().pointerInput(stage) {
                turn(stage, ViewConfiguration.get(context).scaledDoubleTapSlop.toFloat())
            },
            // Transparent, so the badge lies over what is behind it.
            isOpaque = false,
        ) {
            onSurface { surface, width, height ->
                stage.attach(surface, SwapChainFlags.CONFIG_TRANSPARENT, display)
                stage.resize(width, height)
                surface.onChanged { newWidth, newHeight -> stage.resize(newWidth, newHeight) }
                surface.onDestroyed { stage.detach() }
            }
        }
        if (placeholder != null) {
            AnimatedVisibility(
                visible = source != null && !showing,
                modifier = Modifier.matchParentSize(),
                // There from the first frame, and fading once the badge is on
                // screen.
                enter = EnterTransition.None,
                exit = fadeOut(tween(200)),
            ) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    // A failure is told only once it is this source's: the
                    // stage takes a new one after this composition.
                    val mine = failed?.takeIf { stage.source == source && stage.options == options }
                    placeholder(mine ?: LusterState.Minting)
                }
            }
        }
    }
}

/**
 * Turns the badge under one finger: from the moment it lands, so a touch
 * stops a turning badge, by what it moves in points, and with the velocity
 * it lifts at. A second finger makes it a pinch, which brings the badge
 * closer and carries it about; the finger left behind does not turn it. A
 * double tap puts it back. Every change is consumed, so a parent does not
 * scroll with it.
 */
private suspend fun androidx.compose.ui.input.pointer.PointerInputScope.turn(
    stage: LusterStage,
    /** How near a second tap lands to the first for a double tap, pixels, as a View has it. */
    doubleTapSlop: Float,
) {
    var lastTap: Pair<Offset, Long>? = null
    awaitEachGesture {
        val down = awaitFirstDown(requireUnconsumed = false)
        down.consume()
        stage.dragStarted()
        val tracker = VelocityTracker()
        tracker.addPosition(down.uptimeMillis, down.position)
        var last: Offset = down.position
        var lifted = false
        var liftedAt = down.uptimeMillis
        // The two fingers pinching and their spread as they began, while a
        // pinch is going on.
        var pinch: Pinch? = null
        var pinched = false
        try {
            while (true) {
                val event = awaitPointerEvent()
                val pressed = event.changes.filter { it.pressed }
                val pinching = pinch?.takeIf { p -> pressed.any { it.id == p.a } && pressed.any { it.id == p.b } }
                if (pinching == null && pinch != null) {
                    // One of the two lifted: two others still down carry it on.
                    pinch = null
                    stage.pinchEnded()
                }
                if (pinch == null && pressed.size >= 2 && stage.zoomEnabled) {
                    // The drag gives way to the pinch, without a flick.
                    if (!pinched) stage.dragEnded(null)
                    pinched = true
                    pinch = Pinch(pressed[0].id, pressed[1].id, spread(pressed[0], pressed[1]))
                    val middle = middle(pressed[0], pressed[1])
                    stage.pinchStarted(middle.x, middle.y)
                }
                if (pinched) {
                    event.changes.forEach { it.consume() }
                    pinch?.let { p ->
                        val a = pressed.first { it.id == p.a }
                        val b = pressed.first { it.id == p.b }
                        val middle = middle(a, b)
                        stage.pinched(spread(a, b) / p.spread, middle.x, middle.y)
                    }
                    if (pressed.isEmpty()) break
                    continue
                }
                val change = event.changes.firstOrNull { it.id == down.id } ?: break
                tracker.addPosition(change.uptimeMillis, change.position)
                // The lift may come with the last of the movement: it counts too.
                val delta = change.position - last
                last = change.position
                change.consume()
                // Points, as the Apple side measures a drag.
                if (delta != Offset.Zero) stage.dragged(delta.x / density, delta.y / density)
                if (change.changedToUpIgnoreConsumed()) {
                    lifted = true
                    liftedAt = change.uptimeMillis
                    break
                }
            }
        } catch (cancelled: CancellationException) {
            if (pinch != null) stage.pinchEnded()
            stage.dragEnded(null)
            throw cancelled
        }
        if (pinched) return@awaitEachGesture
        stage.dragEnded(if (lifted) tracker.calculateVelocity().x / density else null)
        // A finger lifted where it came down, soon: a second one soon after
        // puts the badge back.
        val timeout = viewConfiguration.doubleTapTimeoutMillis
        if (!lifted || liftedAt - down.uptimeMillis > timeout ||
            (last - down.position).getDistance() > viewConfiguration.touchSlop
        ) return@awaitEachGesture
        val tap = lastTap
        lastTap = if (tap != null && down.uptimeMillis - tap.second <= timeout &&
            (down.position - tap.first).getDistance() <= doubleTapSlop
        ) {
            stage.putBack()
            null
        } else {
            down.position to liftedAt
        }
    }
}

/** The two fingers of a pinch, and how far apart they were as it began. */
private class Pinch(val a: PointerId, val b: PointerId, val spread: Float)

/** The distance between two fingers, pixels. */
private fun spread(a: PointerInputChange, b: PointerInputChange): Float =
    maxOf(1f, (a.position - b.position).getDistance())

/** The point halfway between them. */
private fun middle(a: PointerInputChange, b: PointerInputChange): Offset = (a.position + b.position) / 2f
