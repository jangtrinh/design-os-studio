LazyVerticalGrid(
    columns = object : GridCells { override fun Density.calculateCrossAxisCellSizes(availableSize: Int, spacing: Int): List<Int> = listOf(92.dp, 92.dp, 92.dp, 92.dp).map { it.roundToPx() } },
    modifier = Modifier
        .size(width = 428.dp, height = 164.dp)
        .padding(12.dp),
    horizontalArrangement = Arrangement.spacedBy(12.dp),
    verticalArrangement = Arrangement.spacedBy(12.dp),
    userScrollEnabled = false
) {
    // "HeroTile": row span 2 is not supported by Compose LazyVerticalGrid; placed in row 1
    item(span = { GridItemSpan(2) }) {
        Box(Modifier.height(40.dp), contentAlignment = Alignment.TopStart) {
            Box(
                modifier = Modifier
                    .size(width = 196.dp, height = 92.dp)
                    .background(Color(0xFF3582FF))
            )
        }
    }
    item(span = { GridItemSpan(2) }) {
        Box(Modifier.height(40.dp), contentAlignment = Alignment.TopStart) {
            Box(
                modifier = Modifier
                    .size(width = 196.dp, height = 40.dp)
                    .background(Color(0xFF3582FF))
            )
        }
    }
    item { Spacer(Modifier.height(40.dp)) }
    item { Spacer(Modifier.height(40.dp)) }
    item {
        Box(Modifier.height(40.dp), contentAlignment = Alignment.TopStart) {
            Box(
                modifier = Modifier
                    .size(width = 92.dp, height = 40.dp)
                    .background(Color(0xFF3582FF))
            )
        }
    }
    item {
        Box(Modifier.height(40.dp), contentAlignment = Alignment.TopStart) {
            Box(
                modifier = Modifier
                    .size(width = 92.dp, height = 40.dp)
                    .background(Color(0xFF3582FF))
            )
        }
    }
    item(span = { GridItemSpan(4) }) {
        Box(Modifier.height(36.dp), contentAlignment = Alignment.TopStart) {
            Box(
                modifier = Modifier
                    .size(width = 404.dp, height = 36.dp)
                    .background(Color(0xFF3582FF))
            )
        }
    }
}