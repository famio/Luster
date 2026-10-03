import LusterUI
import SwiftUI

@main
struct LusterDemoApp: App {
    init() { LusterEngine.prewarm() }

    var body: some Scene {
        WindowGroup {
            if CompareView.requested {
                CompareView()
            } else {
                DemoView()
            }
        }
    }
}
