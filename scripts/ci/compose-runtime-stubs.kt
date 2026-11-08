// Minimal androidx.compose.runtime stubs for the Kotlin Compose compiler plugin
// (`kotlin-compose-compiler-plugin.jar` shipped with kotlinc 2.1.10).
//
// Provides the `@Composable` annotation and the runtime entry points targeted
// by `ComposeIrGenerationExtension` so that `@Composable` functions and lambdas
// undergo full K2 FIR frontend checking AND Compose IR lowering.
@file:Suppress("unused", "UNUSED_PARAMETER", "NOTHING_TO_INLINE")

package androidx.compose.runtime

@MustBeDocumented
@Retention(AnnotationRetention.BINARY)
@Target(
    AnnotationTarget.FUNCTION,
    AnnotationTarget.TYPE,
    AnnotationTarget.TYPE_PARAMETER,
    AnnotationTarget.PROPERTY_GETTER,
)
annotation class Composable

@MustBeDocumented
@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.FUNCTION, AnnotationTarget.PROPERTY_GETTER)
annotation class ReadOnlyComposable

@MustBeDocumented
@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.FUNCTION, AnnotationTarget.TYPE, AnnotationTarget.TYPE_PARAMETER)
annotation class DisallowComposableCalls

@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.FUNCTION, AnnotationTarget.PROPERTY_GETTER)
annotation class ExplicitGroupsComposable

@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.FUNCTION, AnnotationTarget.PROPERTY_GETTER)
annotation class NonRestartableComposable

@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.FUNCTION, AnnotationTarget.PROPERTY_GETTER)
annotation class NonSkippableComposable

@Retention(AnnotationRetention.SOURCE)
@Target(
    AnnotationTarget.EXPRESSION,
    AnnotationTarget.FUNCTION,
    AnnotationTarget.PROPERTY_GETTER,
)
annotation class DontMemoize

@Retention(AnnotationRetention.SOURCE)
@Target(
    AnnotationTarget.FUNCTION,
    AnnotationTarget.PROPERTY,
    AnnotationTarget.CLASS,
    AnnotationTarget.FILE,
)
annotation class NoLiveLiterals

@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.ANNOTATION_CLASS, AnnotationTarget.CLASS)
annotation class ComposableTargetMarker(val description: String = "")

@Retention(AnnotationRetention.BINARY)
@Target(
    AnnotationTarget.CLASS,
    AnnotationTarget.FUNCTION,
    AnnotationTarget.PROPERTY_GETTER,
    AnnotationTarget.TYPE,
    AnnotationTarget.TYPE_PARAMETER,
)
annotation class ComposableTarget(val applier: String)

@Retention(AnnotationRetention.BINARY)
@Target(
    AnnotationTarget.FUNCTION,
    AnnotationTarget.PROPERTY_GETTER,
    AnnotationTarget.TYPE,
    AnnotationTarget.TYPE_PARAMETER,
)
annotation class ComposableOpenTarget(val index: Int)

@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.FUNCTION, AnnotationTarget.PROPERTY_GETTER)
annotation class ComposableInferredTarget(val scheme: String)

@MustBeDocumented
@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.ANNOTATION_CLASS, AnnotationTarget.CLASS)
annotation class StableMarker

@MustBeDocumented
@StableMarker
@Retention(AnnotationRetention.BINARY)
@Target(
    AnnotationTarget.CLASS,
    AnnotationTarget.FUNCTION,
    AnnotationTarget.PROPERTY_GETTER,
    AnnotationTarget.PROPERTY,
)
annotation class Stable

@MustBeDocumented
@StableMarker
@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.CLASS)
annotation class Immutable

object ComposeVersion {
    const val version: Int = 12900
}

@Stable
interface State<out T> {
    val value: T
}

interface ScopeUpdateScope {
    fun updateScope(block: (Composer, Int) -> Unit)
}

interface RecomposeScope

interface Applier<N>

interface CompositionTracer

interface Composer {
    val applier: Applier<*>
    val inserting: Boolean
    val skipping: Boolean
    val defaultsInvalid: Boolean
    val recomposeScope: RecomposeScope?
    val recomposeScopeIdentity: Any?
    val compoundKeyHash: Int
    val currentMarker: Int

    fun startReplaceableGroup(key: Int)
    fun endReplaceableGroup()
    fun startReplaceGroup(key: Int)
    fun endReplaceGroup()
    fun startMovableGroup(key: Int, dataKey: Any?)
    fun endMovableGroup()
    fun startDefaults()
    fun endDefaults()
    fun startRestartGroup(key: Int): Composer
    fun endRestartGroup(): ScopeUpdateScope?
    fun sourceInformation(sourceInformation: String)
    fun sourceInformationMarkerStart(key: Int, sourceInformation: String)
    fun sourceInformationMarkerEnd()
    fun skipToGroupEnd()
    fun deactivateToEndGroup(changed: Boolean)
    fun skipCurrentGroup()
    fun endToMarker(marker: Int)
    fun shouldExecute(parametersChanged: Boolean, flags: Int): Boolean = true
    fun joinKey(left: Any?, right: Any?): Any
    fun rememberedValue(): Any?
    fun updateRememberedValue(value: Any?)
    fun changed(value: Any?): Boolean
    fun changed(value: Boolean): Boolean = changed(value as Any?)
    fun changed(value: Char): Boolean = changed(value as Any?)
    fun changed(value: Byte): Boolean = changed(value as Any?)
    fun changed(value: Short): Boolean = changed(value as Any?)
    fun changed(value: Int): Boolean = changed(value as Any?)
    fun changed(value: Float): Boolean = changed(value as Any?)
    fun changed(value: Long): Boolean = changed(value as Any?)
    fun changed(value: Double): Boolean = changed(value as Any?)
    fun changedInstance(value: Any?): Boolean = changed(value)

    companion object {
        val Empty: Any = Any()
    }
}

val currentComposer: Composer
    @ReadOnlyComposable
    @Composable
    get() = throw UnsupportedOperationException("Compose runtime stub")

inline fun <T> Composer.cache(invalid: Boolean, block: @DisallowComposableCalls () -> T): T {
    var result = rememberedValue()
    if (invalid || result === Composer.Empty) {
        result = block()
        updateRememberedValue(result)
    }
    @Suppress("UNCHECKED_CAST")
    return result as T
}

@Composable
inline fun <T> remember(calculation: @DisallowComposableCalls () -> T): T =
    currentComposer.cache(false, calculation)

@Composable
inline fun <T> remember(key1: Any?, calculation: @DisallowComposableCalls () -> T): T =
    currentComposer.cache(currentComposer.changed(key1), calculation)

@Composable
inline fun <T> remember(key1: Any?, key2: Any?, calculation: @DisallowComposableCalls () -> T): T =
    currentComposer.cache(
        currentComposer.changed(key1) or currentComposer.changed(key2),
        calculation,
    )

@Composable
inline fun <T> remember(
    key1: Any?,
    key2: Any?,
    key3: Any?,
    calculation: @DisallowComposableCalls () -> T,
): T =
    currentComposer.cache(
        currentComposer.changed(key1) or
            currentComposer.changed(key2) or
            currentComposer.changed(key3),
        calculation,
    )

@Composable
inline fun <T> remember(
    vararg keys: Any?,
    calculation: @DisallowComposableCalls () -> T,
): T {
    var invalid = false
    for (k in keys) invalid = invalid or currentComposer.changed(k)
    return currentComposer.cache(invalid, calculation)
}

fun isTraceInProgress(): Boolean = false

fun traceEventStart(key: Int, dirty1: Int, dirty2: Int, info: String) {}

@Deprecated("Use 4-arg overload", level = DeprecationLevel.HIDDEN)
fun traceEventStart(key: Int, info: String) {}

fun traceEventEnd() {}

fun sourceInformation(composer: Composer, sourceInformation: String) {}

fun sourceInformationMarkerStart(composer: Composer, key: Int, sourceInformation: String) {}

fun sourceInformationMarkerEnd(composer: Composer) {}

fun updateChangedFlags(flags: Int): Int = flags
