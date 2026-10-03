package dev.famio.luster

import android.content.Context
import android.graphics.Bitmap
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.view.Display
import android.view.Surface
import androidx.annotation.RestrictTo
import dev.famio.luster.core.coast
import dev.famio.luster.core.fieldOfView
import dev.famio.luster.core.filamentCalibration
import dev.famio.luster.core.handling
import dev.famio.luster.core.lamp
import dev.famio.luster.core.lightDirection
import dev.famio.luster.core.lightingPreset
import dev.famio.luster.core.studioCamera
import dev.famio.luster.core.studioLights
import com.google.android.filament.Camera
import com.google.android.filament.Engine
import com.google.android.filament.EntityManager
import com.google.android.filament.IndirectLight
import com.google.android.filament.LightManager
import com.google.android.filament.Renderer
import com.google.android.filament.SwapChain
import com.google.android.filament.SwapChainFlags
import com.google.android.filament.Texture
import com.google.android.filament.TransformManager
import com.google.android.filament.Viewport
import com.google.android.filament.android.DisplayHelper
import com.google.android.filament.gltfio.UbershaderProvider
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import java.nio.ByteBuffer
import kotlin.coroutines.resume
import kotlin.math.cos
import kotlin.math.log2
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin

/**
 * What a badge view draws, whatever kind of view it is: the badge in the
 * studio, minted and re-lit as its settings change, turned by drags measured
 * in points, coasting on, and drawn into whatever surface it is given.
 * [LusterView] and the Compose view are each a surface and a hand on it.
 *
 * Lives on the main thread, where Filament's engine is made. Public for
 * luster-compose only.
 */
@RestrictTo(RestrictTo.Scope.LIBRARY_GROUP)
class LusterStage(private val context: Context) {

    companion object {
        init {
            // Filament's native side, loaded once per process.
            com.google.android.filament.utils.Utils.init()
        }

        // Drag sensitivity (radians per point), the tilt limit, and where the
        // badge rests: the engine's, so a flick feels the same everywhere.
        private val handling = handling()
        val restingPose = floatArrayOf(handling.restingTilt, handling.restingSpin)
        /**
         * What the studio's environment and camera are worth to this
         * renderer: the engine's, matched against the Apple side.
         */
        private val calibration = filamentCalibration()
    }

    /** The document to strike. A different one mints a new badge; the same one does nothing. */
    var source: LusterSource? = null
        set(value) {
            val same = value == field
            field = value
            if (!same && !released) restrike()
        }

    var options: LusterOptions = LusterOptions()
        set(value) {
            val same = value == field
            field = value
            if (!same && !released) restrike()
        }

    var appearance: LusterAppearance = LusterAppearance()
        set(value) {
            val same = value == field
            field = value
            if (!same && !released) relight()
        }

    var onStateChange: ((LusterState) -> Unit)? = null

    var momentumEnabled = true

    /** Set by [release]: the engine is gone, and the setters do nothing. */
    var released = false
        private set

    private val engine = Engine.create()
    private val renderer = engine.createRenderer()
    private val scene = engine.createScene()
    private val view = engine.createView()
    private val cameraEntity = EntityManager.get().create()
    private val camera: Camera = engine.createCamera(cameraEntity)
    private val materials = UbershaderProvider(engine)
    private val displayHelper = DisplayHelper(context)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    private var swapChain: SwapChain? = null
    private var model: BadgeModel? = null
    /** Uploaded once, and lent to every badge. */
    private var grit: Texture? = null
    private var lights = mutableListOf<Int>()
    private var environment: IndirectLight? = null
    private var reflections: Texture? = null
    private var irradiance: Texture? = null
    /** Making the panorama the badge is lit by, once it has been asked for. */
    private var panorama: Job? = null
    private var minting: Job? = null

    private var pose = restingPose.copyOf()
    private var velocity = 0f
    private var dragging = false
    private var lastFrame = 0L

    init {
        view.camera = camera
        view.scene = scene
        camera.setExposure(calibration.aperture, calibration.shutter, calibration.sensitivity)
        // Nothing behind the badge: no skybox, a clear frame, and the view
        // laid over what is under it by its alpha.
        view.blendMode = com.google.android.filament.View.BlendMode.TRANSLUCENT
        renderer.clearOptions = Renderer.ClearOptions().apply {
            clear = true
            clearColor = floatArrayOf(0f, 0f, 0f, 0f)
        }
        relight()
    }

    // MARK: The surface

    /**
     * Draws into [surface] from now on. [flags] are the swap chain's: they
     * must ask for a transparent one, or the clear frame comes out black.
     */
    fun attach(surface: Surface, flags: Long, display: Display?) {
        if (released) return
        swapChain?.let { engine.destroySwapChain(it) }
        swapChain = engine.createSwapChain(surface, flags)
        if (display != null) displayHelper.attach(renderer, display)
    }

    fun resize(width: Int, height: Int) {
        if (released || width <= 0 || height <= 0) return
        view.viewport = Viewport(0, 0, width, height)
        aspect = width.toFloat() / height.toFloat()
        frame()
    }

    /** The surface is going: stop drawing into it, and wait until Filament has. */
    fun detach() {
        if (released) return
        displayHelper.detach()
        swapChain?.let {
            engine.destroySwapChain(it)
            engine.flushAndWait()
            swapChain = null
        }
    }

    /**
     * Advances the coast to [frameTimeNanos] and, when [draw] says the
     * surface is ready, draws a frame. Call it once a frame.
     */
    fun frame(frameTimeNanos: Long, draw: Boolean = true) {
        if (released) return
        val dt = if (lastFrame == 0L) 0f else (frameTimeNanos - lastFrame) / 1_000_000_000f
        lastFrame = frameTimeNanos
        if (!dragging && velocity != 0f) {
            val coast = coast(velocity, dt)
            pose[1] += coast.turn
            velocity = coast.velocity
            turn()
        }
        val chain = swapChain ?: return
        if (draw && renderer.beginFrame(chain, frameTimeNanos)) {
            renderer.render(view)
            renderer.endFrame()
        }
    }

    /** No frame is coming for a while: the next one starts the clock again. */
    fun pause() {
        lastFrame = 0L
    }

    // MARK: The hand

    /** A finger is down: a turning badge stops under it. */
    fun dragStarted() {
        dragging = true
        velocity = 0f
    }

    /** The finger moved by [dx], [dy] points. */
    fun dragged(dx: Float, dy: Float) {
        pose[1] += dx * handling.spinPerPoint
        // Screen y grows downward: dragging down tips the top toward the
        // viewer.
        pose[0] = min(handling.tiltLimit, max(-handling.tiltLimit, pose[0] + dy * handling.tiltPerPoint))
        turn()
    }

    /**
     * The finger is up, moving [velocity] points a second across, or the
     * drag was taken away when it is null: then the badge stays put.
     */
    fun dragEnded(velocity: Float?) {
        dragging = false
        if (velocity != null && momentumEnabled && !reducedMotion()) {
            this.velocity = velocity * handling.spinPerPoint
        }
    }

    /** Whether the system has animations turned off, Android's way of asking for reduced motion. */
    private fun reducedMotion() =
        Settings.Global.getFloat(context.contentResolver,
                                 Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f

    // MARK: The badge

    private fun restrike() {
        val source = source
        minting?.cancel()
        if (source == null) {
            show(null)
            onStateChange?.invoke(LusterState.Idle)
            return
        }
        onStateChange?.invoke(LusterState.Minting)
        minting = scope.launch {
            try {
                val badge = Luster.mint(source, options)
                // The buffers are read off the main thread; Filament's own
                // objects are made on it, where the engine lives.
                val parts = withContext(Dispatchers.Default) { BadgeParts.of(badge.core) }
                show(parts, grit())
                onStateChange?.invoke(LusterState.Ready(badge))
            } catch (cancelled: kotlinx.coroutines.CancellationException) {
                throw cancelled
            } catch (error: Throwable) {
                onStateChange?.invoke(LusterState.Failed(error))
            }
        }
    }

    private fun show(parts: BadgeParts?, grit: Texture? = null) {
        model?.let {
            scene.removeEntities(it.entities)
            it.destroy()
        }
        model = null
        if (parts == null || grit == null) return

        val built = BadgeModel(engine, materials, parts, grit, appearance)
        scene.addEntities(built.entities)
        model = built
        badgeSize = parts.size
        frame()
        turn()
    }

    private suspend fun grit(): Texture {
        grit?.let { return it }
        val pixels = Studio.grit.await()
        // Another badge may have made it meanwhile.
        return grit ?: upload(engine, pixels, srgb = false).also { grit = it }
    }

    // MARK: The studio

    private fun relight() {
        val preset = lightingPreset(appearance.lighting.core())
        model?.dress(appearance)
        turn()
        lights.forEach { entity ->
            scene.removeEntity(entity)
            engine.lightManager.destroy(entity)
            EntityManager.get().destroy(entity)
        }
        lights.clear()

        for (light in studioLights()) {
            val intensity = lamp(light.role, appearance.lighting.core()) * preset.filamentLux
            val direction = lightDirection(light.pitch, light.yaw)
            val entity = EntityManager.get().create()
            LightManager.Builder(LightManager.Type.DIRECTIONAL)
                .color(light.color.r, light.color.g, light.color.b)
                .intensity(intensity)
                .direction(direction.x, direction.y, direction.z)
                .castShadows(false)
                .build(engine, entity)
            scene.addEntity(entity)
            lights.add(entity)
        }

        // Gold is fully metallic: without something to reflect it renders
        // black, so the showcase's panorama is the environment, whatever the
        // lighting makes of it.
        if (panorama == null) {
            panorama = scope.launch {
                val (cube, diffuse) = Studio.panorama.await()
                reflections = cubemap(cube, prefilter = true)
                irradiance = cubemap(diffuse, prefilter = false)
                surround()
            }
        } else {
            surround()
        }
    }

    /** Lights the badge with the room's panorama, at the lighting's strength. */
    private fun surround() {
        val reflections = reflections ?: return
        val irradiance = irradiance ?: return
        val preset = lightingPreset(appearance.lighting.core())
        environment?.let { engine.destroyIndirectLight(it) }
        environment = IndirectLight.Builder()
            .reflections(reflections)
            .irradiance(irradiance)
            .intensity(preset.environment * preset.filamentEnvironmentLux)
            .build(engine)
            .also { scene.indirectLight = it }
    }

    /**
     * A cubemap from the engine's six stacked faces, uploaded as linear
     * floats: Filament will only prefilter floating point, and the same path
     * serves the irradiance faces. With [prefilter] the roughness mips are
     * built as well, which is what a reflection needs.
     */
    private fun cubemap(faces: Faces, prefilter: Boolean): Texture {
        val size = faces.size
        // A view of its own: the faces are shared by every stage.
        val floats = faces.floats.duplicate()
        val offsets = IntArray(6) { it * size * size * 3 * 4 }
        val pixels = Texture.PixelBufferDescriptor(floats, Texture.Format.RGB, Texture.Type.FLOAT)
        val built = Texture.Builder()
            .width(size)
            .height(size)
            .levels(if (prefilter) log2(size.toFloat()).toInt() + 1 else 1)
            .sampler(Texture.Sampler.SAMPLER_CUBEMAP)
            .format(Texture.InternalFormat.RGB16F)
            .build(engine)
        if (prefilter) {
            // The engine's faces are laid out as a cubemap is sampled; Filament
            // mirrors them by default, which would put the studio's lights on
            // the wrong side of the reflection from the irradiance and from
            // what the Apple side and the GLB show.
            val options = Texture.PrefilterOptions().apply { mirror = false }
            built.generatePrefilterMipmap(engine, pixels, offsets, options)
        } else {
            built.setImage(engine, 0, pixels, offsets)
        }
        return built
    }

    // MARK: Framing it

    private var aspect = 1f
    /** What the badge on stage measures across its face, wide by tall. */
    private var badgeSize = 1f to 1f

    /**
     * Points the camera at the badge, wide enough to hold it: the engine's
     * framing, the one every platform uses.
     */
    private fun frame() {
        val setup = studioCamera()
        val fov = fieldOfView(badgeSize.first, badgeSize.second, aspect)
        camera.setProjection(fov.toDouble(), aspect.toDouble(), 0.1, 100.0, Camera.Fov.VERTICAL)
        camera.lookAt(0.0, 0.0, setup.distance.toDouble(), 0.0, 0.0, 0.0, 0.0, 1.0, 0.0)
    }

    // MARK: Turning it

    private fun turn() {
        val model = model ?: return
        val transforms: TransformManager = engine.transformManager
        val instance = transforms.getInstance(model.root)
        transforms.setTransform(instance, rotation(pose[0], pose[1]))
        // The camera looks down -z, so how squarely the reverse faces it is
        // the z of the reverse's normal, -z turned by the pose.
        model.face(-cos(pose[0]) * cos(pose[1]))
    }

    /**
     * Rx(tilt) · Ry(spin), column-major, as Filament wants it: the Apple
     * side's order, so tilt stays about the screen's x and a drag down tips
     * the top toward the viewer even with the badge turned over.
     */
    private fun rotation(tilt: Float, spin: Float): FloatArray {
        val (ct, st) = cos(tilt) to sin(tilt)
        val (cs, ss) = cos(spin) to sin(spin)
        return floatArrayOf(
            cs, st * ss, -ct * ss, 0f,
            0f, ct, st, 0f,
            ss, -st * cs, ct * cs, 0f,
            0f, 0f, 0f, 1f,
        )
    }

    // MARK: Stills

    /**
     * Draws [badge] at [pose] into a square [size] pixels across, as a view
     * shows it, over [background] or over nothing, and reads it back. For a
     * stage with no surface of its own; on the main thread.
     */
    internal suspend fun still(badge: LusterBadge, pose: FloatArray, size: Int,
                               background: LusterColor?): Bitmap {
        minting?.cancel()
        val parts = withContext(Dispatchers.Default) { BadgeParts.of(badge.core) }
        show(parts, grit())
        panorama?.join()
        this.pose = pose.copyOf()
        turn()
        view.viewport = Viewport(0, 0, size, size)
        aspect = 1f
        frame()
        renderer.clearOptions = Renderer.ClearOptions().apply {
            clear = true
            clearColor = background?.let { floatArrayOf(it.red.linear(), it.green.linear(), it.blue.linear(), 1f) }
                ?: floatArrayOf(0f, 0f, 0f, 0f)
        }
        val chain = engine.createSwapChain(size, size,
            SwapChainFlags.CONFIG_TRANSPARENT or SwapChainFlags.CONFIG_READABLE)
        try {
            val pixels = ByteBuffer.allocateDirect(size * size * 4)
            // Filament may ask to skip a frame it thinks would come too soon.
            check((0 until 8).any { renderer.beginFrame(chain, System.nanoTime()) }) {
                "Filament would not draw the still"
            }
            suspendCancellableCoroutine { done ->
                renderer.render(view)
                renderer.readPixels(0, 0, size, size, Texture.PixelBufferDescriptor(
                    pixels, Texture.Format.RGBA, Texture.Type.UBYTE, 1, 0, 0, 0,
                    Handler(Looper.getMainLooper())) { done.resume(Unit) })
                renderer.endFrame()
                // Nothing is drawn, let alone read, until Filament is told to
                // get on with it; the read then lands on the main looper.
                engine.flushAndWait()
            }
            // Top row first, as a bitmap is: Filament turns a read of a
            // headless swap chain right way up itself.
            pixels.rewind()
            // Premultiplied, as Filament blends and as a bitmap keeps it.
            return Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888).apply {
                copyPixelsFromBuffer(pixels)
            }
        } finally {
            engine.destroySwapChain(chain)
            engine.flushAndWait()
        }
    }

    /** Frees the engine. The stage does nothing after. */
    fun release() {
        if (released) return
        released = true
        scope.cancel()
        detach()
        model?.destroy()
        grit?.let { engine.destroyTexture(it) }
        lights.forEach {
            engine.lightManager.destroy(it)
            EntityManager.get().destroy(it)
        }
        environment?.let { engine.destroyIndirectLight(it) }
        reflections?.let { engine.destroyTexture(it) }
        irradiance?.let { engine.destroyTexture(it) }
        engine.destroyRenderer(renderer)
        engine.destroyView(view)
        engine.destroyScene(scene)
        engine.destroyCameraComponent(cameraEntity)
        materials.destroyMaterials()
        engine.destroy()
    }
}
