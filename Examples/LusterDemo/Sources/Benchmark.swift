import Foundation
import LusterUI

/// `-cycle`: mints each of `Sample.benchmarked` a few times through the public
/// API and prints how long each stage took and the longest main-thread frame
/// gap while it ran. The frame gap is what a user would feel as a hitch.
@MainActor
enum Benchmark {
    static var requested: Bool { CommandLine.arguments.contains("-cycle") }

    static func run(monitor: FrameMonitor, show: @escaping (LusterSource, String) -> Void) async {
        setvbuf(stdout, nil, _IOLBF, 0)
        // Let launch settle so its own hitches are not counted.
        try? await Task.sleep(for: .seconds(2))
        print("sample\tmint ms\tgap\tscene ms\tgap\tsettle gap")
        for round in 0..<3 {
            for sample in Sample.benchmarked {
                // A fresh copy of the bytes each round, so the badge cache
                // does not answer: the point is to measure the mint.
                guard var data = try? Data(contentsOf: sample.url) else { continue }
                data.append(contentsOf: Array("<!-- \(round) -->".utf8))
                monitor.reset()
                let t0 = ContinuousClock.now
                guard let badge = try? await LusterEngine.mint(.data(data)) else {
                    print("\(sample.name)\tfailed")
                    continue
                }
                let t1 = ContinuousClock.now
                let minting = monitor.longestGap
                monitor.reset()
                _ = try? await LusterScene.make(badge, appearance: .init())
                let t2 = ContinuousClock.now
                let building = monitor.longestGap
                monitor.reset()
                // A couple of frames after, to catch a hitch at hand-over.
                try? await Task.sleep(for: .milliseconds(100))
                print(String(format: "%@\t%.1f\t%.1f\t%.1f\t%.1f\t%.1f", sample.name,
                             ms(t1 - t0), minting, ms(t2 - t1), building, monitor.longestGap))
                show(.data(data), sample.title)
                try? await Task.sleep(for: .milliseconds(600))
            }
        }
        print("done")
        fflush(stdout)
        // Quit, so that whatever launched the run sees it end.
        exit(0)
    }

    private static func ms(_ d: Duration) -> Double {
        Double(d.components.seconds) * 1000 + Double(d.components.attoseconds) / 1e15
    }
}
