import Luster
import LusterMacKit
import SwiftUI

/// The controls column: metal, lighting, lines, and export.
struct SidebarView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        VStack(spacing: 0) {
            section {
                label("Metal")
                HStack(spacing: 9) {
                    ForEach(AppModel.metals, id: \.name) { metal in
                        metalSwatch(metal.name, metal.color)
                    }
                }
            }

            hairline

            section {
                label("Lighting")
                // Off is for checking a badge's shape: lamps alone, no
                // reflections.
                switchRow("Shine and reflections", isOn: Binding(
                    get: { model.lighting == .showcase },
                    set: { model.lighting = $0 ? .showcase : .off }))
            }

            hairline

            section {
                label("Lines")
                switchRow("Lines and edge top in metal", isOn: $model.metalLines)
            }

            Spacer(minLength: 18)
            hairline

            section {
                Button {
                    model.exportGLB()
                } label: {
                    Text(model.isExporting ? LocalizedStringKey("Exporting…") : "Export GLB")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(model.isExporting)
            }
        }
        .frame(width: Palette.sidebarWidth)
        .background(Palette.sidebar)
    }

    // MARK: Pieces

    private func section<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 11) { content() }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(18)
    }

    private func label(_ text: LocalizedStringKey) -> some View {
        Text(text)
            .font(.system(size: 10, weight: .semibold))
            .kerning(1.4)
            .foregroundStyle(Palette.sectionLabel)
    }

    /// A setting's name with its switch at the column's right edge.
    private func switchRow(_ title: LocalizedStringKey, isOn: Binding<Bool>) -> some View {
        HStack {
            Text(title)
                .font(.system(size: 11))
            Spacer()
            Toggle(title, isOn: isOn)
                .toggleStyle(.switch)
                .tint(Palette.accent)
                .labelsHidden()
        }
    }

    private var hairline: some View {
        Rectangle().fill(Palette.hairline).frame(height: 1)
    }

    private func metalSwatch(_ name: String, _ color: LusterColor) -> some View {
        let selected = model.metal == color
        return Button {
            model.metal = color
        } label: {
            Circle()
                .fill(Color(.sRGB, red: Double(color.red), green: Double(color.green),
                            blue: Double(color.blue)))
                .overlay(Circle().strokeBorder(Palette.swatchEdge, lineWidth: 1))
                .frame(width: 26, height: 26)
                .overlay {
                    // Selection ring, drawn outside the swatch with a gap.
                    if selected {
                        Circle()
                            .strokeBorder(Palette.accent, lineWidth: 1.5)
                            .padding(-3.5)
                    }
                }
        }
        .buttonStyle(.plain)
        .help(Text(LocalizedStringKey(name)))
        .accessibilityLabel(Text("Metal \(Text(LocalizedStringKey(name)))"))
    }
}
