Grid(alignment: .topLeading, horizontalSpacing: 12, verticalSpacing: 12) {
    GridRow {
        // "HeroTile": row span 2 is not supported by SwiftUI Grid; placed in row 1
        Rectangle()
            .fill(Color(red: 0.208, green: 0.51, blue: 1))
            .frame(width: 196, height: 92)
            .frame(width: 196, height: 40, alignment: .topLeading)
            .gridCellColumns(2)
        Rectangle()
            .fill(Color(red: 0.208, green: 0.51, blue: 1))
            .frame(width: 196, height: 40)
            .frame(width: 196, height: 40, alignment: .topLeading)
            .gridCellColumns(2)
    }
    GridRow {
        Color.clear.frame(width: 92, height: 40)
        Color.clear.frame(width: 92, height: 40)
        Rectangle()
            .fill(Color(red: 0.208, green: 0.51, blue: 1))
            .frame(width: 92, height: 40)
            .frame(width: 92, height: 40, alignment: .topLeading)
        Rectangle()
            .fill(Color(red: 0.208, green: 0.51, blue: 1))
            .frame(width: 92, height: 40)
            .frame(width: 92, height: 40, alignment: .topLeading)
    }
    GridRow {
        Rectangle()
            .fill(Color(red: 0.208, green: 0.51, blue: 1))
            .frame(width: 404, height: 36)
            .frame(width: 404, height: 36, alignment: .topLeading)
            .gridCellColumns(4)
    }
}
    .padding(12)
    .frame(width: 428, height: 220, alignment: .topLeading)