package com.example.designos.fixtures

import androidx.compose.runtime.Composable
import androidx.compose.stubs.*

// Scope misuse: Modifier.weight(1f) is only valid in RowScope / ColumnScope, not BoxScope.
@Composable
fun PlantedWeightInBoxScopeFixture() {
    Box(modifier = Modifier.size(width = 320.dp, height = 200.dp)) {
        Box(
            modifier = Modifier
                .weight(1f)
                .height(40.dp)
        )
    }
}
