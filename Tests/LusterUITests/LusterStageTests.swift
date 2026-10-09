import Foundation
import Testing
@testable import LusterUI

/// Momentum has to feel the same on a 60 Hz screen and a 120 Hz one, so it
/// decays by the time passed rather than by the frame.
@MainActor
@Test func momentumFadesAtTheSameRateAtAnyRefreshRate() {
    func spin(frames: Int, per step: Double) -> Float {
        let stage = LusterStage()
        stage.dragChanged(CGSize(width: 40, height: 0))
        stage.dragEnded(velocity: CGSize(width: 900, height: 0))
        for _ in 0..<frames { stage.step(step) }
        return stage.pose.y
    }
    let sixty = spin(frames: 60, per: 1.0 / 60)
    let hundredAndTwenty = spin(frames: 120, per: 1.0 / 120)
    #expect(abs(sixty - hundredAndTwenty) < 0.001)
}

@MainActor
@Test func momentumStops() {
    let stage = LusterStage()
    stage.dragChanged(CGSize(width: 10, height: 0))
    stage.dragEnded(velocity: CGSize(width: 900, height: 0))
    for _ in 0..<600 { stage.step(1.0 / 60) }
    #expect(stage.velocity == 0)
}

@MainActor
@Test func aDragTipsTheBadgeNoFurtherThanTheLimit() {
    let stage = LusterStage()
    stage.dragChanged(CGSize(width: 0, height: 5000))
    #expect(stage.pose.x == LusterStage.tiltLimit)
    stage.dragChanged(CGSize(width: 0, height: -5000))
    #expect(stage.pose.x == -LusterStage.tiltLimit)
}

@MainActor
@Test func turningOffMomentumLeavesTheBadgeWhereItWasLet() {
    let stage = LusterStage()
    stage.momentumEnabled = false
    stage.dragChanged(CGSize(width: 30, height: 0))
    let rested = stage.pose
    stage.dragEnded(velocity: CGSize(width: 1200, height: 0))
    for _ in 0..<30 { stage.step(1.0 / 60) }
    #expect(stage.pose == rested)
}

/// An appearance chosen while a badge is being struck is the one it arrives in,
/// not the one the strike started with.
@MainActor
@Test func anAppearanceChosenWhileMintingIsTheOneTheBadgeArrivesIn() async throws {
    let data = try sampleBadge()
    let stage = LusterStage()
    let shown = LusterAppearance(lighting: .showcase)
    let shape = LusterAppearance(lighting: .off)
    let load = Task { await stage.load(.data(data), options: .init(), appearance: shown) }
    // What `.task(id: appearance)` does when the lighting changes mid-mint.
    while true {
        if case .minting = stage.state { break }
        await Task.yield()
    }
    await stage.apply(shape)
    await load.value
    #expect(stage.scene?.appearance == shape)
}

/// Two badges struck in turn from the same document with different options are
/// two changes, even if nothing but the options tells them apart and the view
/// only looks once both have happened.
@MainActor
@Test func aNewBadgeFromTheSameDocumentIsAChange() async throws {
    let data = try sampleBadge()
    let stage = LusterStage()
    await stage.load(.data(data), options: .init(metalLines: false), appearance: .init())
    let first = (change: stage.change, key: stage.state.badge?.designKey)
    await stage.load(.data(data), options: .init(metalLines: true), appearance: .init())
    #expect(stage.change != first.change)
    #expect(stage.state.badge?.designKey != first.key)
}

/// The placeholder is up while a source has no badge on screen, and is told
/// of a failure only for the source that failed: a new source after a
/// failure is still being struck, though its load has not begun.
@MainActor
@Test func thePlaceholderIsToldOfTheFailureOfItsOwnSource() async throws {
    let broken = LusterSource.svg("<svg xmlns='http://www.w3.org/2000/svg'/>")
    let stage = LusterStage()
    #expect(!stage.showing)
    await stage.load(broken, options: .init(), appearance: .init())
    #expect(!stage.showing)
    guard case .failed = stage.placeholderState(for: broken, options: .init()) else {
        Issue.record("the failure was not passed on")
        return
    }

    let data = try sampleBadge()
    let next = LusterSource.data(data)
    guard case .minting = stage.placeholderState(for: next, options: .init()) else {
        Issue.record("a new source was told of the last one's failure")
        return
    }
    guard case .minting = stage.placeholderState(for: broken, options: .init(metalLines: true))
    else {
        Issue.record("new options were told of the last ones' failure")
        return
    }
    await stage.load(next, options: .init(), appearance: .init())
    #expect(stage.showing)
    await stage.load(nil, options: .init(), appearance: .init())
    #expect(!stage.showing)
}

/// The sample badge, marked with the test asking for it, so that no other
/// test's badge, kept or being struck, is taken for it.
private func sampleBadge(for test: String = #function) throws -> Data {
    let here = URL(fileURLWithPath: #filePath)
    var data = try Data(contentsOf: here.deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("fixtures/svg/sample-badge.svg"))
    data.append(contentsOf: Array("<!-- \(test) -->".utf8))
    return data
}
