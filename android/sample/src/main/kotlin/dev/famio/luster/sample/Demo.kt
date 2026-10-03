package dev.famio.luster.sample

import android.os.SystemClock
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.TextAutoSize
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import dev.famio.luster.LusterBadge
import dev.famio.luster.LusterSnapshot
import dev.famio.luster.LusterSource
import dev.famio.luster.LusterState
import dev.famio.luster.LusterView as LusterViewClass
import dev.famio.luster.compose.LusterView
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

/**
 * The demo: the badge above, and below it the buttons that open another SVG,
 * set the lines, the metal and the light, and save the badge as a GLB, a
 * status line, and two tabs. The tabs change only how the badge is drawn,
 * Compose (`dev.famio.luster.compose.LusterView`) or View (`dev.famio.luster.LusterView`
 * in an `AndroidView`); the document and the settings are the same in both,
 * so the two can be compared as they are.
 */
@Composable
fun Demo(state: DemoState, dark: Boolean, tab: Launch.Tab, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var shown by remember { mutableStateOf(tab) }
    var status by remember { mutableStateOf("idle") }
    /** The badge on screen, once it is ready: what the GLB button writes out. */
    var ready by remember { mutableStateOf<LusterBadge?>(null) }
    var saving by remember { mutableStateOf(false) }
    var started by remember { mutableLongStateOf(0L) }
    val gaps = remember { FrameGaps() }
    DisposableEffect(gaps) {
        gaps.start()
        onDispose { gaps.stop() }
    }
    val openSvg = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) scope.launch {
            try {
                val (bytes, name) = readSvg(context, uri)
                state.source = LusterSource.bytes(bytes)
                state.title = name
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Exception) {
                status = "failed: ${error.message}"
            }
        }
    }
    val saveGlb = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("model/gltf-binary"),
    ) { uri ->
        val badge = ready
        if (uri == null || badge == null) {
            status = "not saved"
            return@rememberLauncherForActivityResult
        }
        val name = state.title
        val metal = state.settings.metal
        saving = true
        scope.launch {
            try {
                val size = writeGlb(context, uri, badge, metal)
                status = "exported $name.glb · ${size / 1024} KB"
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Exception) {
                status = "failed: ${error.message}"
            } finally {
                saving = false
            }
        }
    }

    /** What either view reports: the status line, and the badge to write out. */
    val onState: (LusterState) -> Unit = { badgeState ->
        ready = (badgeState as? LusterState.Ready)?.badge
        status = when (badgeState) {
            is LusterState.Idle -> "idle"
            is LusterState.Minting -> {
                gaps.reset()
                started = SystemClock.elapsedRealtime()
                "minting…"
            }
            is LusterState.Ready -> "ready ${badgeState.badge.designKey.take(6)} · " +
                "${SystemClock.elapsedRealtime() - started} ms · longest frame gap ${gaps.longest} ms"
            is LusterState.Failed -> "failed: ${badgeState.error}"
        }
    }

    val foreground = if (dark) Color.White else Color.Black
    Column(modifier.background(if (dark) Color.Black else Color.White)) {
        // One view or the other, never both: leaving, a view frees its engine.
        key(shown) {
            val badge = Modifier.weight(1f).fillMaxWidth()
            when (shown) {
                Launch.Tab.COMPOSE -> LusterView(
                    source = state.source,
                    modifier = badge,
                    options = state.settings.options,
                    appearance = state.settings.appearance,
                    onStateChange = onState,
                )
                Launch.Tab.VIEW -> AndroidView(
                    factory = { LusterViewClass(it) },
                    modifier = badge,
                    // Setting what is already set does nothing.
                    update = {
                        it.onStateChange = onState
                        it.options = state.settings.options
                        it.appearance = state.settings.appearance
                        it.source = state.source
                    },
                    onRelease = { it.release() },
                )
            }
        }
        Row(
            Modifier.fillMaxWidth().padding(start = 16.dp, top = 16.dp, end = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            BarButton("SVG", state.title, R.drawable.ic_folder, dark, foreground, Modifier.weight(1f)) {
                openSvg.launch(arrayOf("image/svg+xml"))
            }
            for (menu in DemoMenu.all) {
                MenuButton(menu, state, dark, foreground, Modifier.weight(1f))
            }
            BarButton("GLB", "GLB", R.drawable.ic_export, dark, foreground, Modifier.weight(1f),
                      enabled = ready != null && !saving) {
                status = "saving ${state.title}.glb…"
                saveGlb.launch("${state.title}.glb")
            }
        }
        Text(status, Modifier.padding(start = 16.dp, top = 8.dp, end = 16.dp, bottom = 8.dp),
             fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = Color(0xFF8E8E93),
             maxLines = 1, overflow = TextOverflow.MiddleEllipsis)
        NavigationBar {
            for (each in Launch.Tab.entries) {
                NavigationBarItem(
                    selected = shown == each,
                    onClick = { shown = each },
                    icon = {},
                    label = { Text(each.title) },
                )
            }
        }
    }
}

/** A grey capsule with an icon over a caption, like the iOS demo's buttons. */
@Composable
private fun BarButton(
    /** What the button is: its accessibility name. */
    title: String,
    caption: String,
    icon: Int,
    dark: Boolean,
    foreground: Color,
    modifier: Modifier,
    enabled: Boolean = true,
    onClick: () -> Unit,
) {
    Column(
        modifier
            .alpha(if (enabled) 1f else 0.35f)
            .clip(CircleShape)
            .background(if (dark) Color(0xFF2C2C2E) else Color(0xFFE5E5EA))
            .clickable(enabled = enabled, onClick = onClick)
            .semantics { contentDescription = title }
            .heightIn(min = 56.dp)
            .padding(horizontal = 4.dp, vertical = 8.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Icon(painterResource(icon), contentDescription = null, tint = foreground,
             modifier = Modifier.size(22.dp))
        // A longer name ("Showcase") shrinks a little rather than being cut.
        BasicText(
            caption,
            Modifier.padding(top = 3.dp),
            style = TextStyle(color = foreground, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center),
            maxLines = 1,
            autoSize = TextAutoSize.StepBased(minFontSize = 8.sp, maxFontSize = 11.sp, stepSize = 1.sp),
        )
    }
}

/** A button that opens `menu`, its choices checked as they are set. */
@Composable
private fun MenuButton(menu: DemoMenu, state: DemoState, dark: Boolean, foreground: Color,
                       modifier: Modifier) {
    var open by remember { mutableStateOf(false) }
    Box(modifier) {
        BarButton(menu.title, menu.current(state.settings), menu.icon, dark, foreground,
                  Modifier.fillMaxWidth()) { open = true }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            for (item in menu.items) {
                DropdownMenuItem(
                    text = { Text(if (item.isOn(state.settings)) "✓  ${item.title}" else "    ${item.title}") },
                    onClick = {
                        state.settings = item.apply(state.settings)
                        open = false
                    },
                )
            }
        }
    }
}

/** The bundled samples `--ez list true` sets in a list, by asset name and title. */
private val listed = listOf(
    "namiura" to "Namiura",
    "sample-badge" to "Seal",
    "pin-with-stroke" to "Pin",
    "fuji" to "Fuji",
    "many-colour-cells" to "Colour wheel",
    "wreathed-emblem" to "Wreath",
)

/** Badges in a scrolling list: a drag on one turns it, a swipe beside it scrolls. */
@Composable
fun BadgeList(settings: DemoSettings, stills: Boolean = false, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    LazyColumn(modifier.fillMaxSize()) {
        items(listed) { (name, title) ->
            Text(title, Modifier.padding(start = 16.dp, top = 24.dp), fontSize = 16.sp,
                 color = Color(0xFF8E8E93))
            // Narrower than the list, so there is room beside it to scroll.
            Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                val svg = remember(name) { sample(context, name) }
                if (stills) {
                    val still by produceState<ImageBitmap?>(null, name, settings) {
                        value = LusterSnapshot.bitmap(context, svg, settings.options, settings.appearance,
                                                      pixelSize = 520).asImageBitmap()
                    }
                    still?.let { Image(it, title, Modifier.size(260.dp)) }
                        ?: Box(Modifier.size(260.dp))
                } else {
                    LusterView(
                        source = svg,
                        modifier = Modifier.size(260.dp),
                        options = settings.options,
                        appearance = settings.appearance,
                    )
                }
            }
        }
    }
}
