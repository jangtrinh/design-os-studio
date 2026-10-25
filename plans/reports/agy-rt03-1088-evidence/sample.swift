import SwiftUI

struct Button: View {
    let label: String
    let disabled: Bool
    var body: some View { EmptyView() }
}

enum Tokens {
    static let spacingMd: CGFloat = 12
}

struct GeneratedCardSample: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("cost ${y} and \\(x)")
                .font(.custom("Inter", size: 20))
                .fontWeight(.semibold)
                .foregroundColor(Color(red: 0.067, green: 0.067, blue: 0.067))
            Text("Say \"hi\" \\ path\nline2\ttab\rcr\u{1} 🚀")
                .font(.custom("Inter", size: 14))
                .foregroundColor(Color("textMuted"))
                .multilineTextAlignment(.center)
                .lineSpacing(6)
                .frame(maxWidth: .infinity)
            HStack(spacing: Tokens.spacingMd) {
                Spacer()
                Button(label: "Pay ${amount} (\"now\")\n\t\\(fast)\u{7} 🚀", disabled: true)
                    .frame(width: 80, height: 32)
                Circle()
                    .fill(Color(red: 1, green: 0, blue: 0))
                    .frame(width: 24, height: 24)
                Spacer()
            }
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .frame(maxWidth: .infinity)
                .background(RoundedRectangle(cornerRadius: 4).fill(Color("primary")))
        }
            .padding(16)
            .frame(width: 312, height: 200, alignment: .topLeading)
            .background(RoundedRectangle(cornerRadius: 8).fill(Color.white))
            .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color(red: 0.898, green: 0.898, blue: 0.898), lineWidth: 1))
            .clipShape(RoundedRectangle(cornerRadius: 8))
            .shadow(color: Color.black.opacity(0.25), radius: 4, x: 0, y: 4)
    }
}
