import SwiftUI

struct PlantedBrokenSwiftUIView: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Planted broken SwiftUI fixture")
                .frame(maxWidth: .infinity, height: 44)
                .nonExistentDesignOsModifier(42)
        }
    }
}
