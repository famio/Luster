package dev.famio.luster.sample

import androidx.annotation.DrawableRes
import dev.famio.luster.LusterAppearance
import dev.famio.luster.LusterColor
import dev.famio.luster.LusterLighting
import dev.famio.luster.LusterOptions

/** Everything the demo's menus set. */
data class DemoSettings(
    /** The lines and the edge's top left in the metal rather than plated in the art's colours. */
    val metalLines: Boolean = false,
    val metal: LusterColor = LusterColor.Gold,
    val lighting: LusterLighting = LusterLighting.SHOWCASE,
) {
    val options get() = LusterOptions(metalLines = metalLines)

    val appearance get() = LusterAppearance(metal, lighting)
}

/**
 * One of the demo's menus: what it is called, its choices, and which one is
 * set. The same menus, with the same choices, as the iOS and Flutter demos.
 * The SVG and the GLB are not menus: one has a button that opens a file, the
 * other a button that saves one.
 */
class DemoMenu(val title: String, @DrawableRes val icon: Int, val items: List<Item>) {
    class Item(
        val title: String,
        val isOn: (DemoSettings) -> Boolean,
        val apply: (DemoSettings) -> DemoSettings,
    )

    /** What the menu's button shows: the choice that is set. */
    fun current(settings: DemoSettings) = items.firstOrNull { it.isOn(settings) }?.title ?: "—"

    companion object {
        val lines = choices(
            "Lines", R.drawable.ic_lines, { it.metalLines }, { copy(metalLines = it) },
            listOf("Plated" to false, "Metal" to true),
        )

        val metal = choices(
            "Metal", R.drawable.ic_metal, { it.metal }, { copy(metal = it) },
            listOf(
                "Gold" to LusterColor.Gold,
                "Silver" to LusterColor.Silver,
                "Copper" to LusterColor.Copper,
            ),
        )

        val light = choices(
            "Light", R.drawable.ic_light, { it.lighting }, { copy(lighting = it) },
            listOf(
                "Showcase" to LusterLighting.SHOWCASE,
                "Off" to LusterLighting.OFF,
            ),
        )

        val all = listOf(lines, metal, light)

        private fun <T> choices(
            title: String,
            @DrawableRes icon: Int,
            get: (DemoSettings) -> T,
            set: DemoSettings.(T) -> DemoSettings,
            values: List<Pair<String, T>>,
        ) = DemoMenu(title, icon, values.map { (name, value) ->
            Item(name, isOn = { get(it) == value }, apply = { it.set(value) })
        })
    }
}

/**
 * How the demo was opened. Set from adb, so a still can be taken the same way
 * twice:
 *
 *     adb shell am start -n dev.famio.luster.sample/.MainActivity --es tab view \
 *         --ez bare true --es sample fuji --es light Off --es metal Silver --es lines Metal
 *
 * `tab` is compose (the default) or view. `bare` shows that tab's badge alone,
 * filling the window; `list` sets badges in a scrolling list. The others pick
 * a sample from the assets and a choice from each menu, by the name it shows.
 */
data class Launch(
    val tab: Tab,
    val bare: Boolean,
    val list: Boolean,
    /** The list drawn as stills (LusterSnapshot) rather than live views. */
    val stills: Boolean,
    val sample: String,
    /** A web document to open on instead of [sample]. */
    val url: String?,
    val settings: DemoSettings,
) {
    enum class Tab(val title: String) { COMPOSE("Compose"), VIEW("View") }

    companion object {
        fun of(intent: android.content.Intent): Launch {
            var settings = DemoSettings()
            for (menu in DemoMenu.all) {
                val wanted = intent.getStringExtra(menu.title.lowercase()) ?: continue
                val item = menu.items.firstOrNull { it.title.equals(wanted, ignoreCase = true) } ?: continue
                settings = item.apply(settings)
            }
            return Launch(
                tab = if (intent.getStringExtra("tab") == "view") Tab.VIEW else Tab.COMPOSE,
                bare = intent.getBooleanExtra("bare", false),
                list = intent.getBooleanExtra("list", false),
                stills = intent.getBooleanExtra("stills", false),
                sample = intent.getStringExtra("sample") ?: DemoState.SAMPLE,
                url = intent.getStringExtra("url"),
                settings = settings,
            )
        }
    }
}
