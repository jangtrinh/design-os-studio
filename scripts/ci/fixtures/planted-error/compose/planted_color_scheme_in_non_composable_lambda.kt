package com.example.designos.planted

import androidx.compose.runtime.Composable
import androidx.compose.stubs.*

// Planted-error fixture: reads `MaterialTheme.colorScheme` (which is a
// `@Composable @ReadOnlyComposable` getter in real Material 3) directly inside
// `LazyVerticalGrid`'s non-composable `LazyGridScope.() -> Unit` lambda.
@Composable
fun PlantedColorSchemeInNonComposableLambda() {
    LazyVerticalGrid(
        columns = GridCells.Fixed(2),
        modifier = Modifier.size(width = 320.dp, height = 200.dp),
    ) {
        val accent = MaterialTheme.colorScheme.primary
        item {
            Box(
                modifier = Modifier
                    .size(width = 120.dp, height = 40.dp)
                    .background(accent),
            )
        }
    }
}
