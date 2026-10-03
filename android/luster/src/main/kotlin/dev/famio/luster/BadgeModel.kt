package dev.famio.luster

import dev.famio.luster.core.Material
import dev.famio.luster.core.MaterialRole
import dev.famio.luster.core.filamentBackRoughness
import dev.famio.luster.core.finish
import dev.famio.luster.core.sandblast
import dev.famio.luster.core.vertexLayout
import com.google.android.filament.Box
import com.google.android.filament.Engine
import com.google.android.filament.EntityManager
import com.google.android.filament.IndexBuffer
import com.google.android.filament.MaterialInstance
import com.google.android.filament.RenderableManager
import com.google.android.filament.SurfaceOrientation
import com.google.android.filament.Texture
import com.google.android.filament.TextureSampler
import com.google.android.filament.VertexBuffer
import com.google.android.filament.gltfio.MaterialProvider
import com.google.android.filament.gltfio.UbershaderProvider
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.max
import kotlin.math.sqrt

/*
 * A badge in Filament, built from the engine's own buffers: a renderable per
 * submesh under one root, shaded with gltfio's ubershaders and set up the way
 * gltfio sets up a glTF, so it looks as its GLB would, without writing one
 * and reading it back. Textures go up as the RGBA they are, never through
 * PNG.
 *
 * The work is split as on the Apple side: [BadgeParts] is made off the main
 * thread, and [BadgeModel] builds Filament's objects on the engine's thread.
 */

/** Byte offsets into a vertex, as `luster-core` lays it out. */
private val layout = vertexLayout()
private val POSITION_OFFSET = layout.position.toInt()
private val NORMAL_OFFSET = layout.normal.toInt()
private val UV_OFFSET = layout.uv.toInt()
private val TANGENT_OFFSET = layout.tangent.toInt()

/** RGBA8, rows top to bottom, in a buffer Filament can read. */
internal class Pixels(val width: Int, val height: Int, val rgba: ByteBuffer) {
    companion object {
        fun of(texture: dev.famio.luster.core.Texture) = Pixels(
            texture.width.toInt(),
            texture.height.toInt(),
            direct(texture.rgba),
        )

        fun grit() = of(sandblast())
    }
}

/** One submesh, ready to upload. */
internal class MeshParts(
    val material: Int,
    val vertexCount: Int,
    val indexCount: Int,
    /** The engine's interleaved vertices, as they are. */
    val vertices: ByteBuffer,
    /** Each vertex's normal and tangent as one quaternion, what Filament lights with. */
    val orientation: ByteBuffer,
    val indices: ByteBuffer,
    val box: Box,
)

/** Everything a badge needs before Filament sees it. */
internal class BadgeParts(
    val meshes: List<MeshParts>,
    val materials: List<Material>,
    val textures: List<Pixels>,
) {
    /** What the badge measures across its face, wide by tall. */
    val size: Pair<Float, Float> by lazy {
        val low = floatArrayOf(Float.MAX_VALUE, Float.MAX_VALUE)
        val high = floatArrayOf(-Float.MAX_VALUE, -Float.MAX_VALUE)
        for (mesh in meshes) {
            val centre = mesh.box.center
            val half = mesh.box.halfExtent
            for (axis in 0 until 2) {
                low[axis] = minOf(low[axis], centre[axis] - half[axis])
                high[axis] = maxOf(high[axis], centre[axis] + half[axis])
            }
        }
        if (meshes.isEmpty()) 1f to 1f else (high[0] - low[0]) to (high[1] - low[1])
    }

    companion object {
        /** Reads the badge's buffers into what Filament takes. Blocks: call it off the main thread. */
        fun of(badge: dev.famio.luster.core.LusterBadge): BadgeParts {
            val stride = layout.stride.toInt()
            val meshes = badge.submeshes().filter { it.indexCount > 0u }.map { submesh ->
                val count = submesh.vertexCount.toInt()
                val vertices = direct(submesh.vertices)
                val floats = vertices.duplicate().order(ByteOrder.LITTLE_ENDIAN)

                // SurfaceOrientation takes its inputs packed, one after another.
                val normals = floatBuffer(count * 3)
                val tangents = floatBuffer(count * 4)
                val low = floatArrayOf(Float.MAX_VALUE, Float.MAX_VALUE, Float.MAX_VALUE)
                val high = floatArrayOf(-Float.MAX_VALUE, -Float.MAX_VALUE, -Float.MAX_VALUE)
                for (vertex in 0 until count) {
                    val at = vertex * stride
                    for (axis in 0 until 3) {
                        val p = floats.getFloat(at + POSITION_OFFSET + 4 * axis)
                        low[axis] = minOf(low[axis], p)
                        high[axis] = maxOf(high[axis], p)
                        normals.put(floats.getFloat(at + NORMAL_OFFSET + 4 * axis))
                    }
                    for (k in 0 until 4) {
                        tangents.put(floats.getFloat(at + TANGENT_OFFSET + 4 * k))
                    }
                }
                normals.rewind()
                tangents.rewind()
                val orientation = ByteBuffer.allocateDirect(count * 8).order(ByteOrder.nativeOrder())
                val surface = SurfaceOrientation.Builder()
                    .vertexCount(count)
                    .normals(normals)
                    .tangents(tangents)
                    .build()
                surface.getQuatsAsShort(orientation)
                surface.destroy()
                orientation.rewind()

                MeshParts(
                    material = submesh.material.toInt(),
                    vertexCount = count,
                    indexCount = submesh.indexCount.toInt(),
                    vertices = vertices,
                    orientation = orientation,
                    indices = direct(submesh.indices),
                    box = Box(
                        (low[0] + high[0]) / 2, (low[1] + high[1]) / 2, (low[2] + high[2]) / 2,
                        (high[0] - low[0]) / 2, (high[1] - low[1]) / 2, (high[2] - low[2]) / 2,
                    ),
                )
            }
            return BadgeParts(meshes, badge.materials(), badge.textures().map(Pixels::of))
        }
    }
}

/**
 * The badge's Filament objects. Made and destroyed on the engine's thread;
 * [root] is what turns it.
 */
internal class BadgeModel(
    private val engine: Engine,
    provider: UbershaderProvider,
    parts: BadgeParts,
    /** The sandblast, which the view keeps for every badge. */
    grit: Texture,
    appearance: LusterAppearance,
) {
    val root: Int = EntityManager.get().create()
    val entities: IntArray

    private val textures: List<Texture> = parts.textures.map { upload(engine, it, srgb = true) }
    private val instances: List<MaterialInstance>
    /** What each instance is for, which decides how the appearance dresses it. */
    private val roles: List<MaterialRole> = parts.materials.map { it.role }
    private val vertexBuffers = mutableListOf<VertexBuffer>()
    private val indexBuffers = mutableListOf<IndexBuffer>()
    private var filler: ByteBuffer? = null

    /** The appearance last dressed in, which the reverse follows as it turns. */
    private var appearance = appearance

    init {
        engine.transformManager.create(root)
        instances = parts.materials.map { material ->
            instance(provider, material, material.texture?.let { textures[it.toInt()] },
                     grit.takeIf { material.role == MaterialRole.GOLD_BACK })
        }
        dress(appearance)
        entities = parts.meshes.map { mesh -> renderable(mesh) }.toIntArray()
    }

    /**
     * Plates the badge in the appearance's metal and gives each material the
     * sheen its lighting asks for. The engine says how each is finished, as
     * it does for the Apple side; the enamel and painted faces keep their
     * colours.
     */
    fun dress(appearance: LusterAppearance) {
        this.appearance = appearance
        val metal = appearance.metal
        for ((instance, role) in instances.zip(roles)) {
            val finish = finish(role, appearance.lighting.core())
            if (finish.plated) {
                instance.setParameter("baseColorFactor",
                                      metal.red.linear(), metal.green.linear(), metal.blue.linear(), 1f)
            }
            instance.setParameter("metallicFactor", finish.metallic)
            // Filament's own roughness: its image-based light blurs more
            // than RealityKit's at the same value.
            instance.setParameter("roughnessFactor", finish.filamentRoughness)
            // Filament's reflectance is glass's 4% at 0.5 and goes as its
            // square; the engine's is a part of glass's.
            instance.setParameter("reflectance", 0.5f * sqrt(finish.specular))
        }
    }

    /**
     * Gives the reverse the roughness Filament wants for how squarely it
     * faces the camera, 1 head on and 0 edge on (or turned away): its
     * reflection does not spread as RealityKit's does when it turns edge on,
     * and the engine says how much rougher to make it instead.
     */
    fun face(facing: Float) {
        val roughness = filamentBackRoughness(appearance.lighting.core(), facing)
        for ((instance, role) in instances.zip(roles)) {
            if (role == MaterialRole.GOLD_BACK) instance.setParameter("roughnessFactor", roughness)
        }
    }

    /**
     * An instance of the ubershader a glTF material of these properties is
     * given, with the parameters gltfio sets for it.
     */
    private fun instance(
        provider: UbershaderProvider,
        material: Material,
        texture: Texture?,
        normal: Texture?,
    ): MaterialInstance {
        val key = MaterialProvider.MaterialKey().apply {
            hasBaseColorTexture = texture != null
            hasNormalTexture = normal != null
        }
        val instance = provider.createMaterialInstance(key, IntArray(8), material.name, null)
            ?: error("no ubershader for ${material.name}")
        // A textured face carries the document's own colours; its factor
        // must not tint them.
        val tint = if (texture != null) floatArrayOf(1f, 1f, 1f) else floatArrayOf(
            material.color.r.linear(), material.color.g.linear(), material.color.b.linear())
        instance.setParameter("baseColorFactor", tint[0], tint[1], tint[2], 1f)
        instance.setParameter("metallicFactor", material.metallic)
        instance.setParameter("roughnessFactor", material.roughness)
        instance.setParameter("emissiveFactor", 0f, 0f, 0f)
        // The grit at full strength, as RealityKit and the GLB have it.
        instance.setParameter("normalScale", 1f)
        instance.setParameter("aoStrength", 1f)
        if (instance.material.hasParameter("emissiveStrength")) {
            instance.setParameter("emissiveStrength", 1f)
        }
        // glTF's default sampling, with the mipmaps gltfio makes.
        val sampler = TextureSampler(
            TextureSampler.MinFilter.LINEAR_MIPMAP_LINEAR,
            TextureSampler.MagFilter.LINEAR,
            TextureSampler.WrapMode.REPEAT,
        )
        texture?.let { instance.setParameter("baseColorMap", it, sampler) }
        normal?.let { instance.setParameter("normalMap", it, sampler) }
        return instance
    }

    private fun renderable(mesh: MeshParts): Int {
        val stride = layout.stride.toInt()
        // The ubershaders ask for a second UV set and a vertex colour; like
        // gltfio, one buffer of 0xff stands in for both.
        val vertices = VertexBuffer.Builder()
            .bufferCount(3)
            .vertexCount(mesh.vertexCount)
            .attribute(VertexBuffer.VertexAttribute.POSITION, 0,
                       VertexBuffer.AttributeType.FLOAT3, POSITION_OFFSET, stride)
            .attribute(VertexBuffer.VertexAttribute.UV0, 0,
                       VertexBuffer.AttributeType.FLOAT2, UV_OFFSET, stride)
            .attribute(VertexBuffer.VertexAttribute.TANGENTS, 1,
                       VertexBuffer.AttributeType.SHORT4, 0, 8)
            .normalized(VertexBuffer.VertexAttribute.TANGENTS)
            .attribute(VertexBuffer.VertexAttribute.UV1, 2,
                       VertexBuffer.AttributeType.USHORT2, 0, 4)
            .normalized(VertexBuffer.VertexAttribute.UV1)
            .attribute(VertexBuffer.VertexAttribute.COLOR, 2,
                       VertexBuffer.AttributeType.UBYTE4, 0, 4)
            .normalized(VertexBuffer.VertexAttribute.COLOR)
            .build(engine)
        vertices.setBufferAt(engine, 0, mesh.vertices)
        vertices.setBufferAt(engine, 1, mesh.orientation)
        vertices.setBufferAt(engine, 2, filler(mesh.vertexCount))
        vertexBuffers += vertices

        val indices = IndexBuffer.Builder()
            .indexCount(mesh.indexCount)
            .bufferType(IndexBuffer.Builder.IndexType.UINT)
            .build(engine)
        indices.setBuffer(engine, mesh.indices)
        indexBuffers += indices

        val entity = EntityManager.get().create()
        RenderableManager.Builder(1)
            .boundingBox(mesh.box)
            .geometry(0, RenderableManager.PrimitiveType.TRIANGLES, vertices, indices)
            .material(0, instances[mesh.material])
            .culling(true)
            .castShadows(true)
            .receiveShadows(true)
            .build(engine, entity)
        val transforms = engine.transformManager
        transforms.create(entity, transforms.getInstance(root), null as FloatArray?)
        return entity
    }

    /** 0xff for `count` vertices, four bytes each; one buffer, grown as needed. */
    private fun filler(count: Int): ByteBuffer {
        val wanted = count * 4
        val have = filler
        if (have != null && have.capacity() >= wanted) {
            return have.duplicate().apply { limit(wanted) }
        }
        val made = ByteBuffer.allocateDirect(wanted)
        while (made.hasRemaining()) made.put(0xff.toByte())
        made.rewind()
        filler = made
        return made.duplicate()
    }

    /** Everything but the grit, which belongs to the view. */
    fun destroy() {
        val entityManager = EntityManager.get()
        for (entity in entities) {
            engine.destroyEntity(entity)
            entityManager.destroy(entity)
        }
        engine.destroyEntity(root)
        entityManager.destroy(root)
        vertexBuffers.forEach(engine::destroyVertexBuffer)
        indexBuffers.forEach(engine::destroyIndexBuffer)
        instances.forEach(engine::destroyMaterialInstance)
        textures.forEach(engine::destroyTexture)
    }
}

/**
 * A texture with its whole mip chain, as gltfio makes one: sRGB for colour,
 * linear for a normal map.
 */
internal fun upload(engine: Engine, pixels: Pixels, srgb: Boolean): Texture {
    val texture = Texture.Builder()
        .width(pixels.width)
        .height(pixels.height)
        .levels(levels(pixels.width, pixels.height))
        .sampler(Texture.Sampler.SAMPLER_2D)
        .format(if (srgb) Texture.InternalFormat.SRGB8_A8 else Texture.InternalFormat.RGBA8)
        .usage(Texture.Usage.DEFAULT or Texture.Usage.GEN_MIPMAPPABLE)
        .build(engine)
    texture.setImage(engine, 0, Texture.PixelBufferDescriptor(
        pixels.rgba.duplicate(), Texture.Format.RGBA, Texture.Type.UBYTE))
    texture.generateMipmaps(engine)
    return texture
}

private fun levels(width: Int, height: Int): Int {
    var size = max(width, height)
    var count = 1
    while (size > 1) {
        size /= 2
        count++
    }
    return count
}

private fun direct(bytes: ByteArray): ByteBuffer =
    ByteBuffer.allocateDirect(bytes.size).order(ByteOrder.nativeOrder()).apply {
        put(bytes)
        rewind()
    }

private fun floatBuffer(count: Int) =
    ByteBuffer.allocateDirect(count * 4).order(ByteOrder.nativeOrder()).asFloatBuffer()
