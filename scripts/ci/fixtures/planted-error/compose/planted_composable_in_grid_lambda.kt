package com.example.designos.fixtures

import androidx.compose.runtime.Composable
import androidx.compose.stubs.*

// Reproduces #1450: calling a @Composable directly inside LazyVerticalGrid's
// non-composable LazyGridScope.() -> Unit lambda without wrapping in item { ... }.
@Composable
fun PlantedComposableInGridLambdaFixture() {
    LazyVerticalGrid(
        columns = GridCells.Fixed(2),
        modifier = Modifier.size(width = 320.dp, height = 200.dp)
    ) {
        Box(modifier = Modifier.size(width = 120.dp, height = 40.dp))
        Text(text = "Direct composable call in non-composable grid lambda")
    }
}
