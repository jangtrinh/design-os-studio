package com.example.designos.fixtures

import androidx.compose.runtime.Composable
import androidx.compose.stubs.*

@Composable
fun PlantedBrokenComposeFixture() {
    Column(
        modifier = Modifier
            .width(320.dp)
            .nonExistentModifierCall(123)
    ) {
        Text("Planted broken Compose fixture", fontSize = "invalid_font_size")
    }
}
