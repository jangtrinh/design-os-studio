@file:Suppress("UNUSED_PARAMETER", "unused", "FunctionName", "ClassName", "PropertyName")

package androidx.compose.stubs

import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable

// ============================================================================
// Part 1: Faithful Jetpack Compose UI, Foundation, and Material 3 API surface
// ============================================================================

@DslMarker
annotation class LayoutScopeMarker

class Dp(val value: Float)

val Int.dp: Dp get() = Dp(this.toFloat())
val Double.dp: Dp get() = Dp(this.toFloat())
val Float.dp: Dp get() = Dp(this)

class TextUnit(val value: Float)

val Int.sp: TextUnit get() = TextUnit(this.toFloat())
val Double.sp: TextUnit get() = TextUnit(this.toFloat())
val Float.sp: TextUnit get() = TextUnit(this)

interface Density {
    fun Dp.roundToPx(): Int = value.toInt()
}

class Color {
    val value: Long
    constructor(value: Long) {
        this.value = value
    }
    constructor(value: Int) {
        this.value = value.toLong()
    }

    companion object {
        val Transparent = Color(0x00000000)
        val Black = Color(0xFF000000)
        val White = Color(0xFFFFFFFF)
    }
}

class Offset(val x: Float = 0f, val y: Float = 0f) {
    companion object {
        val Zero = Offset(0f, 0f)
        val Infinite = Offset(Float.POSITIVE_INFINITY, Float.POSITIVE_INFINITY)
    }
}

open class Brush {
    companion object {
        fun linearGradient(
            vararg colorStops: Pair<Float, Color>,
            start: Offset = Offset.Zero,
            end: Offset = Offset.Infinite,
        ): Brush = Brush()

        fun linearGradient(
            colors: List<Color>,
            start: Offset = Offset.Zero,
            end: Offset = Offset.Infinite,
        ): Brush = Brush()

        fun radialGradient(
            vararg colorStops: Pair<Float, Color>,
            center: Offset = Offset.Zero,
            radius: Float = Float.POSITIVE_INFINITY,
        ): Brush = Brush()

        fun radialGradient(
            colors: List<Color>,
            center: Offset = Offset.Zero,
            radius: Float = Float.POSITIVE_INFINITY,
        ): Brush = Brush()

        fun sweepGradient(
            vararg colorStops: Pair<Float, Color>,
            center: Offset = Offset.Zero,
        ): Brush = Brush()

        fun sweepGradient(
            colors: List<Color>,
            center: Offset = Offset.Zero,
        ): Brush = Brush()
    }
}

class SolidColor(val value: Color) : Brush()

interface Shape
object RectangleShape : Shape
object CircleShape : Shape

fun RoundedCornerShape(size: Dp): Shape = RectangleShape
fun RoundedCornerShape(
    topStart: Dp = 0.dp,
    topEnd: Dp = 0.dp,
    bottomEnd: Dp = 0.dp,
    bottomStart: Dp = 0.dp,
): Shape = RectangleShape

interface Alignment {
    interface Horizontal
    interface Vertical

    companion object {
        val TopStart: Alignment = object : Alignment {}
        val TopCenter: Alignment = object : Alignment {}
        val TopEnd: Alignment = object : Alignment {}
        val CenterStart: Alignment = object : Alignment {}
        val Center: Alignment = object : Alignment {}
        val CenterEnd: Alignment = object : Alignment {}
        val BottomStart: Alignment = object : Alignment {}
        val BottomCenter: Alignment = object : Alignment {}
        val BottomEnd: Alignment = object : Alignment {}

        val Top: Vertical = object : Vertical {}
        val CenterVertically: Vertical = object : Vertical {}
        val Bottom: Vertical = object : Vertical {}

        val Start: Horizontal = object : Horizontal {}
        val CenterHorizontally: Horizontal = object : Horizontal {}
        val End: Horizontal = object : Horizontal {}
    }
}

object Arrangement {
    interface Horizontal
    interface Vertical
    interface HorizontalOrVertical : Horizontal, Vertical

    val Start: Horizontal = object : Horizontal {}
    val End: Horizontal = object : Horizontal {}
    val Top: Vertical = object : Vertical {}
    val Bottom: Vertical = object : Vertical {}
    val Center: HorizontalOrVertical = object : HorizontalOrVertical {}
    val SpaceBetween: HorizontalOrVertical = object : HorizontalOrVertical {}
    val SpaceAround: HorizontalOrVertical = object : HorizontalOrVertical {}
    val SpaceEvenly: HorizontalOrVertical = object : HorizontalOrVertical {}

    fun spacedBy(space: Dp): HorizontalOrVertical = object : HorizontalOrVertical {}
    fun spacedBy(space: Dp, alignment: Alignment.Horizontal): Horizontal = object : Horizontal {}
    fun spacedBy(space: Dp, alignment: Alignment.Vertical): Vertical = object : Vertical {}
}

class PaddingValues(
    val start: Dp = 0.dp,
    val top: Dp = 0.dp,
    val end: Dp = 0.dp,
    val bottom: Dp = 0.dp,
) {
    constructor(all: Dp) : this(all, all, all, all)
    constructor(horizontal: Dp = 0.dp, vertical: Dp = 0.dp) : this(horizontal, vertical, horizontal, vertical)
}

// Note: `weight` and `align` are intentionally NOT members of `Modifier`.
// They exist only as scope-bound extensions on `RowScope`, `ColumnScope`, and `BoxScope`.
interface Modifier {
    fun width(width: Dp): Modifier = this
    fun height(height: Dp): Modifier = this
    fun size(size: Dp): Modifier = this
    fun size(width: Dp, height: Dp): Modifier = this
    fun widthIn(min: Dp = 0.dp, max: Dp = Dp(Float.POSITIVE_INFINITY)): Modifier = this
    fun heightIn(min: Dp = 0.dp, max: Dp = Dp(Float.POSITIVE_INFINITY)): Modifier = this
    fun fillMaxWidth(fraction: Float = 1f): Modifier = this
    fun fillMaxHeight(fraction: Float = 1f): Modifier = this
    fun fillMaxSize(fraction: Float = 1f): Modifier = this
    fun wrapContentWidth(
        align: Alignment.Horizontal = Alignment.CenterHorizontally,
        unbounded: Boolean = false,
    ): Modifier = this
    fun wrapContentHeight(
        align: Alignment.Vertical = Alignment.CenterVertically,
        unbounded: Boolean = false,
    ): Modifier = this
    fun wrapContentSize(
        align: Alignment = Alignment.Center,
        unbounded: Boolean = false,
    ): Modifier = this
    fun padding(all: Dp): Modifier = this
    fun padding(horizontal: Dp = 0.dp, vertical: Dp = 0.dp): Modifier = this
    fun padding(
        start: Dp = 0.dp,
        top: Dp = 0.dp,
        end: Dp = 0.dp,
        bottom: Dp = 0.dp,
    ): Modifier = this
    fun padding(paddingValues: PaddingValues): Modifier = this
    fun offset(x: Dp = 0.dp, y: Dp = 0.dp): Modifier = this
    fun background(color: Color, shape: Shape = RectangleShape): Modifier = this
    fun background(brush: Brush, shape: Shape = RectangleShape, alpha: Float = 1.0f): Modifier = this
    fun border(width: Dp, color: Color, shape: Shape = RectangleShape): Modifier = this
    fun border(width: Dp, brush: Brush, shape: Shape = RectangleShape): Modifier = this
    fun clip(shape: Shape): Modifier = this
    fun clipToBounds(): Modifier = this
    fun shadow(
        elevation: Dp,
        shape: Shape = RectangleShape,
        clip: Boolean = false,
        ambientColor: Color = Color.Black,
        spotColor: Color = Color.Black,
    ): Modifier = this
    fun blur(radius: Dp): Modifier = this
    fun alpha(alpha: Float): Modifier = this
    fun rotate(degrees: Float): Modifier = this
    fun zIndex(zIndex: Float): Modifier = this

    companion object : Modifier
}

class FontWeight(val weight: Int) {
    companion object {
        val Thin = FontWeight(100)
        val ExtraLight = FontWeight(200)
        val Light = FontWeight(300)
        val Normal = FontWeight(400)
        val Medium = FontWeight(500)
        val SemiBold = FontWeight(600)
        val Bold = FontWeight(700)
        val ExtraBold = FontWeight(800)
        val Black = FontWeight(900)
    }
}

enum class FontStyle { Normal, Italic }
enum class TextAlign { Left, Right, Center, Justify, Start, End }

class TextDecoration {
    companion object {
        val None = TextDecoration()
        val Underline = TextDecoration()
        val LineThrough = TextDecoration()
    }
}

enum class TextOverflow { Clip, Ellipsis, Visible }

class Font(
    val resId: Int,
    val weight: FontWeight = FontWeight.Normal,
    val style: FontStyle = FontStyle.Normal,
)

class FontFamily(vararg val fonts: Font) {
    constructor(fonts: List<Font>) : this(*fonts.toTypedArray())

    companion object {
        val Default = FontFamily()
        val SansSerif = FontFamily()
        val Serif = FontFamily()
        val Monospace = FontFamily()
    }
}

class Painter

@Composable
fun painterResource(id: Int): Painter = Painter()

class ColorScheme(
    val primary: Color = Color(0xFF0D99FF),
    val secondary: Color = Color(0xFF625B71),
    val background: Color = Color(0xFFFFFFFF),
    val surface: Color = Color(0xFFFFFFFF),
    val error: Color = Color(0xFFB3261E),
    val onPrimary: Color = Color(0xFFFFFFFF),
    val onSecondary: Color = Color(0xFFFFFFFF),
    val onBackground: Color = Color(0xFF1C1B1F),
    val onSurface: Color = Color(0xFF1C1B1F),
)

object MaterialTheme {
    val colorScheme: ColorScheme
        @Composable
        @ReadOnlyComposable
        get() = ColorScheme()
}

@LayoutScopeMarker
interface RowScope {
    fun Modifier.weight(weight: Float, fill: Boolean = true): Modifier = this
    fun Modifier.align(alignment: Alignment.Vertical): Modifier = this
}

object RowScopeInstance : RowScope

@LayoutScopeMarker
interface ColumnScope {
    fun Modifier.weight(weight: Float, fill: Boolean = true): Modifier = this
    fun Modifier.align(alignment: Alignment.Horizontal): Modifier = this
}

object ColumnScopeInstance : ColumnScope

@LayoutScopeMarker
interface BoxScope {
    fun Modifier.align(alignment: Alignment): Modifier = this
    fun Modifier.matchParentSize(): Modifier = this
}

object BoxScopeInstance : BoxScope

@Composable
fun Row(
    modifier: Modifier = Modifier,
    horizontalArrangement: Arrangement.Horizontal = Arrangement.Start,
    verticalAlignment: Alignment.Vertical = Alignment.Top,
    content: @Composable RowScope.() -> Unit = {},
) {
    RowScopeInstance.content()
}

@Composable
fun Column(
    modifier: Modifier = Modifier,
    verticalArrangement: Arrangement.Vertical = Arrangement.Top,
    horizontalAlignment: Alignment.Horizontal = Alignment.Start,
    content: @Composable ColumnScope.() -> Unit = {},
) {
    ColumnScopeInstance.content()
}

@Composable
fun Box(
    modifier: Modifier = Modifier,
    contentAlignment: Alignment = Alignment.TopStart,
    propagateMinConstraints: Boolean = false,
    content: @Composable BoxScope.() -> Unit = {},
) {
    BoxScopeInstance.content()
}

@Composable
fun Spacer(modifier: Modifier = Modifier) {}

@Composable
fun Text(
    text: String,
    modifier: Modifier = Modifier,
    color: Color = Color.Black,
    fontSize: TextUnit = 14.sp,
    fontStyle: FontStyle? = null,
    fontWeight: FontWeight? = null,
    fontFamily: FontFamily? = null,
    letterSpacing: TextUnit = 0.sp,
    textDecoration: TextDecoration? = null,
    textAlign: TextAlign? = null,
    lineHeight: TextUnit = 20.sp,
    overflow: TextOverflow = TextOverflow.Clip,
    softWrap: Boolean = true,
    maxLines: Int = Int.MAX_VALUE,
) {}

@Composable
fun Icon(
    painter: Painter,
    contentDescription: String?,
    modifier: Modifier = Modifier,
    tint: Color = Color.Black,
) {}

class GridItemSpan(val currentLineSpan: Int)

@LayoutScopeMarker
interface LazyGridItemSpanScope {
    val maxLineSpan: Int get() = 1
    val maxCurrentLineSpan: Int get() = 1
}

object LazyGridItemSpanScopeInstance : LazyGridItemSpanScope

@LayoutScopeMarker
interface LazyGridItemScope
object LazyGridItemScopeInstance : LazyGridItemScope

interface GridCells {
    class Fixed(val count: Int) : GridCells
    class Adaptive(val minSize: Dp) : GridCells
    class FixedSize(val size: Dp) : GridCells

    fun Density.calculateCrossAxisCellSizes(availableSize: Int, spacing: Int): List<Int> = emptyList()
}

@LayoutScopeMarker
interface LazyGridScope {
    fun item(
        key: Any? = null,
        span: (LazyGridItemSpanScope.() -> GridItemSpan)? = null,
        contentType: Any? = null,
        content: @Composable LazyGridItemScope.() -> Unit,
    )
}

object LazyGridScopeInstance : LazyGridScope {
    override fun item(
        key: Any?,
        span: (LazyGridItemSpanScope.() -> GridItemSpan)?,
        contentType: Any?,
        content: @Composable LazyGridItemScope.() -> Unit,
    ) {
        span?.invoke(LazyGridItemSpanScopeInstance)
    }
}

@Composable
fun LazyVerticalGrid(
    columns: GridCells,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(0.dp),
    reverseLayout: Boolean = false,
    verticalArrangement: Arrangement.Vertical = Arrangement.Top,
    horizontalArrangement: Arrangement.Horizontal = Arrangement.Start,
    userScrollEnabled: Boolean = true,
    content: LazyGridScope.() -> Unit,
) {
    LazyGridScopeInstance.content()
}

// ============================================================================
// Part 2: User-supplied project placeholders (design tokens, R resources,
// and custom component stubs referenced by the test fixtures)
// ============================================================================

val ColorScheme.text: Color get() = Color(0xFF111827)
val ColorScheme.textMuted: Color get() = Color(0xFF6B7280)

object R {
    object font {
        const val inter: Int = 1
        const val roboto: Int = 2
        const val sf_pro_text: Int = 3
        const val sf_pro_display: Int = 4
    }

    object drawable {
        const val ic_placeholder: Int = 1
    }
}

object Tokens {
    val md: Dp = 12.dp
    val sm: Dp = 8.dp
    val lg: Dp = 16.dp
    val xl: Dp = 24.dp
    val spacingXs: Dp = 4.dp
    val spacingSm: Dp = 8.dp
    val spacingMd: Dp = 12.dp
    val spacingLg: Dp = 16.dp
    val spacingXl: Dp = 24.dp
    val primary: Color = Color(0xFF0D99FF)
    val surface: Color = Color(0xFFFFFFFF)
    val text: Color = Color(0xFF111827)
    val textMuted: Color = Color(0xFF6B7280)
}

@Composable
fun ActionButton(
    label: String = "",
    disabled: Boolean = false,
    modifier: Modifier = Modifier,
    onClick: () -> Unit = {},
) {}
