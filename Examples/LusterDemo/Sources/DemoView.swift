import LusterUI
import SwiftUI
import UniformTypeIdentifiers

/// The demo: the badge above, and below it the buttons that open an SVG, set
/// the lines, the metal and the light, and save the badge as a GLB, a status
/// line, and two tabs. The tabs change only how the badge is drawn, SwiftUI
/// (`LusterView`) or UIKit (`LusterUIView`); the document and the settings are
/// the same in both, so the two can be compared as they are. `-tab uikit`
/// opens on the second.
struct DemoView: View {
    /// How the badge is drawn: the one tab or the other.
    enum Renderer: String, CaseIterable, Identifiable {
        case swiftUI = "SwiftUI", uiKit = "UIKit"
        var id: Self { self }
        var symbol: String { self == .swiftUI ? "swift" : "square.stack" }

        static var starting: Renderer {
            let arguments = CommandLine.arguments
            guard let index = arguments.firstIndex(of: "-tab"), index + 1 < arguments.endIndex
            else { return .swiftUI }
            return arguments[index + 1].lowercased() == "uikit" ? .uiKit : .swiftUI
        }
    }

    @State private var renderer = Renderer.starting
    @State private var settings = DemoSettings()
    @State private var status = "idle"
    @State private var importing = false
    /// The badge on screen, once it is ready: what the GLB button writes out.
    @State private var badge: LusterBadge?
    @State private var glb: GLBFile?
    @State private var exporting = false
    @State private var monitor = FrameMonitor()
    @State private var started = Date()

    var body: some View {
        VStack(spacing: 0) {
            // One view or the other, never both: leaving, a view frees its engine.
            Group {
                switch renderer {
                case .swiftUI:
                    // The status line says why a badge failed.
                    LusterView(source: settings.source, options: settings.options,
                               appearance: settings.appearance, onStateChange: report) { state in
                        if case .minting = state { ProgressView().controlSize(.large) }
                    }
                case .uiKit:
                    LusterUIViewPanel(settings: settings, onState: report)
                }
            }
            .id(renderer)
            VStack(alignment: .leading, spacing: 8) {
                DemoMenuBar(settings: $settings, open: { importing = true },
                            export: badge.map { badge in { export(badge) } })
                Text(status).font(.caption.monospaced()).foregroundStyle(.secondary)
                    .lineLimit(1).truncationMode(.middle)
                    .accessibilityIdentifier("status")
            }
            .padding()
            tabBar
            // Not on the same view as the importer: SwiftUI shows one file
            // panel per view.
            .fileExporter(isPresented: $exporting, document: glb, contentType: .glb,
                          defaultFilename: settings.exportName) { result in
                switch result {
                case .success:
                    if let glb { status = DemoSettings.exported(glb.data, as: settings.exportName) }
                case .failure(CocoaError.userCancelled): status = DemoSettings.notSaved
                case .failure(let error): status = "failed: \(error.localizedDescription)"
                }
                glb = nil
            }
            // Swiping the sheet away closes it without a word to the handler
            // above. A save may be told of after the sheet closes, so only a
            // sheet still waiting is taken as dismissed.
            .onChange(of: exporting) {
                if !exporting, status == DemoSettings.saving(as: settings.exportName) {
                    status = DemoSettings.notSaved
                }
            }
        }
        .onAppear { monitor.start() }
        .task {
            if Benchmark.requested {
                await Benchmark.run(monitor: monitor) { settings.show($0, titled: $1) }
            }
        }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.svg]) { result in
            if case .success(let url) = result { settings.open(url) }
        }
    }

    /// What either view reports: the status line, and the badge to write out.
    private func report(_ state: LusterState) {
        badge = nil
        switch state {
        case .idle: status = "idle"
        case .minting:
            monitor.reset()
            started = Date()
            status = "minting…"
        case .ready(let badge):
            self.badge = badge
            let ms = Date().timeIntervalSince(started) * 1000
            status = String(format: "ready %@ · %.0f ms · longest frame gap %.0f ms",
                            String(badge.designKey.prefix(6)), ms, monitor.longestGap)
        case .failed(let error): status = "failed: \(error)"
        }
    }

    /// The two tabs, which choose the view that draws the badge.
    private var tabBar: some View {
        HStack {
            ForEach(Renderer.allCases) { each in
                Button {
                    renderer = each
                } label: {
                    VStack(spacing: 2) {
                        Image(systemName: each.symbol).font(.title3)
                        Text(each.rawValue).font(.caption2)
                    }
                    .frame(maxWidth: .infinity, minHeight: 44)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .foregroundStyle(renderer == each ? Color.accentColor : .secondary)
                .accessibilityIdentifier(each.rawValue)
                .accessibilityAddTraits(renderer == each ? .isSelected : [])
            }
        }
        .padding(.top, 6)
        .background(.bar)
    }

    /// Writes `badge` out, then asks where to save it.
    private func export(_ badge: LusterBadge) {
        Task {
            glb = GLBFile(await settings.glb(of: badge))
            status = DemoSettings.saving(as: settings.exportName)
            exporting = true
        }
    }
}

/// The button that opens an SVG, showing the one on the badge, then the demo's
/// menus in a row, each showing what it has set, then the button that saves a
/// GLB.
struct DemoMenuBar: View {
    @Binding var settings: DemoSettings
    let open: () -> Void
    /// Nil until there is a badge to save.
    let export: (() -> Void)?

    var body: some View {
        HStack(spacing: 4) {
            Button(action: open) {
                label(DemoMenu.openSymbol, settings.sourceTitle)
            }
            .buttonStyle(.bordered)
            .tint(.primary)
            .accessibilityIdentifier(DemoMenu.openTitle)
            ForEach(DemoMenu.all) { menu in
                Menu {
                    Section(menu.title) {
                        ForEach(menu.items) { item in
                            Button {
                                item.apply(&settings)
                            } label: {
                                if item.isOn(settings) {
                                    Label(item.title, systemImage: "checkmark")
                                } else {
                                    Text(item.title)
                                }
                            }
                        }
                    }
                } label: {
                    label(menu.symbol, menu.current(settings))
                }
                .menuStyle(.button)
                .buttonStyle(.bordered)
                .tint(.primary)
                .accessibilityIdentifier(menu.title)
            }
            Button {
                export?()
            } label: {
                label(DemoMenu.exportSymbol, DemoMenu.exportTitle)
            }
            .buttonStyle(.bordered)
            .tint(.primary)
            .disabled(export == nil)
            .accessibilityIdentifier(DemoMenu.exportTitle)
        }
    }

    private func label(_ symbol: String, _ title: String) -> some View {
        VStack(spacing: 3) {
            Image(systemName: symbol).font(.body)
            // A longer name ("Showcase") shrinks a little rather than being cut.
            Text(title).font(.caption2).lineLimit(1).minimumScaleFactor(0.8)
        }
        .frame(maxWidth: .infinity, minHeight: 44)
        .contentShape(Rectangle())
    }
}

/// `LusterUIView` in SwiftUI, set from the demo's settings.
struct LusterUIViewPanel: UIViewRepresentable {
    let settings: DemoSettings
    let onState: @MainActor (LusterState) -> Void

    func makeUIView(context: Context) -> LusterUIView {
        let view = LusterUIView(source: settings.source, appearance: settings.appearance)
        view.options = settings.options
        let spinner = UIActivityIndicatorView(style: .large)
        spinner.startAnimating()
        view.placeholderView = spinner
        view.onStateChange = report(to: view)
        return view
    }

    func updateUIView(_ view: LusterUIView, context: Context) {
        // Setting what is already set does nothing.
        view.source = settings.source
        view.options = settings.options
        view.badgeAppearance = settings.appearance
        view.onStateChange = report(to: view)
    }

    /// Stops the spinner on a failure, as the SwiftUI tab shows nothing then.
    /// The view itself takes it away as the badge arrives.
    private func report(to view: LusterUIView) -> @MainActor (LusterState) -> Void {
        { [onState, weak view] state in
            if let spinner = view?.placeholderView as? UIActivityIndicatorView {
                switch state {
                case .minting: spinner.startAnimating()
                case .failed: spinner.stopAnimating()
                case .idle, .ready: break
                }
            }
            onState(state)
        }
    }
}
