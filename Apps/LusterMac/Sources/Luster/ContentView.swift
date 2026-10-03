import Luster
import LusterMacKit
import LusterUI
import SwiftUI

struct ContentView: View {
    @ObservedObject var model: AppModel
    @State private var isDropTarget = false

    var body: some View {
        VStack(spacing: 0) {
            titleBar
            Rectangle().fill(Palette.hairline).frame(height: 1)
            HStack(spacing: 0) {
                stage
                Rectangle().fill(Palette.sidebarEdge).frame(width: 1)
                SidebarView(model: model)
            }
        }
        .background(Palette.window)
        // Swatches and tiles draw their own selection ring; the system focus
        // ring would duplicate it.
        .focusEffectDisabled()
        .frame(minWidth: 900, minHeight: 600)
        .alert(item: $model.alert) { alert in
            Alert(title: Text(alert.title), message: Text(alert.message),
                  dismissButton: .cancel(Text("OK")))
        }
    }

    // MARK: Title bar

    private var titleBar: some View {
        // Centered on the whole bar, not on the space beside the traffic lights.
        Text(verbatim: "LUSTER")
            .font(.system(size: 11, weight: .semibold))
            .kerning(2.4)
            .foregroundStyle(Palette.titleText)
            .frame(maxWidth: .infinity)
            .frame(height: Palette.titleBarHeight)
            .background(Palette.titleBar)
    }

    // MARK: Stage

    private var stage: some View {
        ZStack {
            LusterView(source: model.svgData.map { .data($0) },
                           options: model.options,
                           appearance: model.appearance) { state in
                switch state {
                case let .ready(badge): model.badge = badge
                case let .failed(error): model.report(error)
                case .idle, .minting: break
                }
            }
            if model.svgData == nil {
                Text("Open an SVG to make a 3D badge.")
                    .font(.system(size: 12))
                    .foregroundStyle(Palette.fileText)
            }

            VStack {
                Spacer()
                HStack(alignment: .bottom) {
                    Text(model.fileName)
                        .font(.system(size: 10.5, design: .monospaced))
                        .kerning(0.4)
                        .foregroundStyle(Palette.fileText)
                        .padding(.leading, 22)
                        .padding(.bottom, 18)
                    Spacer()
                    Button {
                        model.pickFile()
                    } label: {
                        Label("Open SVG", systemImage: "square.and.arrow.up")
                    }
                    .controlSize(.large)
                    .padding(.trailing, 20)
                        .padding(.bottom, 16)
                }
            }
            .allowsHitTesting(true)

            if isDropTarget { dropVeil }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Palette.window)
        .onDrop(of: SVGDrop.acceptedTypes, isTargeted: $isDropTarget) { providers in
            model.receive(providers)
        }
    }

    private var dropVeil: some View {
        ZStack {
            Palette.dropVeil
            RoundedRectangle(cornerRadius: 16)
                .strokeBorder(Palette.accent.opacity(0.55),
                              style: StrokeStyle(lineWidth: 1, dash: [6, 5]))
                .frame(width: 210, height: 210)
                .overlay {
                    Text("Drop an SVG")
                        .font(.system(size: 12))
                        .kerning(0.7)
                        .foregroundStyle(Palette.dropText)
                }
        }
        .allowsHitTesting(false)
    }
}
