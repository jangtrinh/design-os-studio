// Minimal androidx.compose.runtime.internal stubs for the Kotlin Compose compiler plugin
// (`kotlin-compose-compiler-plugin.jar` shipped with kotlinc 2.1.10).
//
// Provides `ComposableLambda`, `composableLambda`, `rememberComposableLambda`,
// `StabilityInferred`, `FunctionKeyMeta`, and live-literal helpers required by
// `ComposeIrGenerationExtension` during lambda memoization and IR lowering.
@file:Suppress("unused", "UNUSED_PARAMETER")

package androidx.compose.runtime.internal

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Composer
import androidx.compose.runtime.Stable
import androidx.compose.runtime.State
import kotlin.jvm.functions.FunctionN

@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.CLASS)
annotation class StabilityInferred(val parameters: Int)

@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.FUNCTION, AnnotationTarget.CONSTRUCTOR)
annotation class FunctionKeyMeta(
    val key: Int,
    val startOffset: Int,
    val endOffset: Int,
)

@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.CLASS)
annotation class FunctionKeyMetaClass(val file: String)

@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.CLASS)
annotation class LiveLiteralFileInfo(val file: String)

@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.FUNCTION)
annotation class LiveLiteralInfo(val key: String, val offset: Int)

@Stable
interface ComposableLambda :
    Function2<Composer, Int, Any?>,
    Function3<Any?, Composer, Int, Any?>,
    Function4<Any?, Any?, Composer, Int, Any?>,
    Function5<Any?, Any?, Any?, Composer, Int, Any?>,
    Function6<Any?, Any?, Any?, Any?, Composer, Int, Any?>,
    Function7<Any?, Any?, Any?, Any?, Any?, Composer, Int, Any?>,
    Function8<Any?, Any?, Any?, Any?, Any?, Any?, Composer, Int, Any?>,
    Function9<Any?, Any?, Any?, Any?, Any?, Any?, Any?, Composer, Int, Any?>,
    Function10<Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Composer, Int, Any?>,
    Function11<Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Composer, Int, Any?>,
    Function13<Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Composer, Int, Int, Any?>,
    Function14<Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Composer, Int, Int, Any?>,
    Function15<Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Composer, Int, Int, Any?>,
    Function16<Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Composer, Int, Int, Any?>,
    Function17<Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Composer, Int, Int, Any?>,
    Function18<Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Composer, Int, Int, Any?>,
    Function19<Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Composer, Int, Int, Any?>,
    Function20<Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Composer, Int, Int, Any?>,
    Function21<Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Any?, Composer, Int, Int, Any?>

@Stable
interface ComposableLambdaN : FunctionN<Any?>

fun composableLambda(
    composer: Composer,
    key: Int,
    tracked: Boolean,
    block: Any,
): ComposableLambda = throw UnsupportedOperationException("Compose runtime stub")

fun composableLambdaInstance(
    key: Int,
    tracked: Boolean,
    block: Any,
): ComposableLambda = throw UnsupportedOperationException("Compose runtime stub")

@Composable
fun rememberComposableLambda(
    key: Int,
    tracked: Boolean,
    block: Any,
): ComposableLambda = composableLambdaInstance(key, tracked, block)

fun composableLambdaN(
    composer: Composer,
    key: Int,
    tracked: Boolean,
    arity: Int,
    block: Any,
): ComposableLambdaN = throw UnsupportedOperationException("Compose runtime stub")

fun composableLambdaNInstance(
    key: Int,
    tracked: Boolean,
    arity: Int,
    block: Any,
): ComposableLambdaN = throw UnsupportedOperationException("Compose runtime stub")

@Composable
fun rememberComposableLambdaN(
    key: Int,
    tracked: Boolean,
    arity: Int,
    block: Any,
): ComposableLambdaN = composableLambdaNInstance(key, tracked, arity, block)

fun isLiveLiteralsEnabled(): Boolean = false

fun <T> liveLiteral(key: String, value: T): State<T> =
    object : State<T> {
        override val value: T = value
    }
