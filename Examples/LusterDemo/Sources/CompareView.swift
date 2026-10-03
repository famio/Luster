import LusterUI
import SwiftUI
import UIKit

/// `-compare`: LusterView above and LusterUIView below, the same size and
/// the same settings, to see that the two draw and handle alike. The SVG
/// button steps through the bundled samples; `-sample <name>` opens on one.
/// `-only-swiftui` or `-only-uiview` shows one alone, the same size as the
/// other would be: in the simulator, a second RealityKit view on screen is
/// drawn late, whichever kind it is, so stills to compare are taken alone.
struct CompareView: View {
    static var requested: Bool { CommandLine.arguments.contains("-compare") }

    @State private var settings = DemoSettings.compared
    @State private var swiftUIStatus = "idle"
    @State private var uiKitStatus = "idle"

    var body: some View {
        VStack(spacing: 0) {
            let arguments = CommandLine.arguments
            if !arguments.contains("-only-uiview") {
                panel("LusterView (SwiftUI)", status: swiftUIStatus) {
                    LusterView(source: settings.source, options: settings.options,
                               appearance: settings.appearance) { swiftUIStatus = describe($0) }
                }
            }
            if !arguments.contains("-only-uiview") && !arguments.contains("-only-swiftui") {
                Divider()
            }
            if !arguments.contains("-only-swiftui") {
                panel("LusterUIView (ARView)", status: uiKitStatus) {
                    LusterUIViewPanel(settings: settings) { uiKitStatus = describe($0) }
                }
            }
            DemoMenuBar(settings: $settings, open: nextSample, export: nil)
                .padding()
        }
    }

    private func panel(_ title: String, status: String,
                       @ViewBuilder content: () -> some View) -> some View {
        content()
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .overlay(alignment: .topLeading) {
                VStack(alignment: .leading) {
                    Text(title).font(.caption.bold())
                    Text(status).font(.caption2.monospaced()).foregroundStyle(.secondary)
                }
                .padding(8)
                .allowsHitTesting(false)
            }
    }

    private func nextSample() {
        let samples = Sample.benchmarked
        guard !samples.isEmpty else { return }
        let index = samples.firstIndex { settings.source == .url($0.url) } ?? -1
        let next = samples[(index + 1) % samples.count]
        settings.source = .url(next.url)
        settings.sourceTitle = next.title
    }

    private func describe(_ state: LusterState) -> String {
        switch state {
        case .idle: "idle"
        case .minting: "minting…"
        case .ready(let badge): "ready \(badge.designKey.prefix(6))"
        case .failed(let error): "failed: \(error)"
        }
    }
}

extension DemoSettings {
    /// The demo's settings, opening on `-sample <name>` when it is given.
    static var compared: DemoSettings {
        var settings = DemoSettings()
        let arguments = CommandLine.arguments
        if let index = arguments.firstIndex(of: "-sample"), index + 1 < arguments.endIndex,
           let sample = Sample.benchmarked.first(where: { $0.name == arguments[index + 1] }) {
            settings.source = .url(sample.url)
            settings.sourceTitle = sample.title
        }
        return settings
    }
}
