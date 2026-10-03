import Foundation
import RealityKit
import Testing
@testable import Luster

/// Under the showcase the enamel keeps half its glare, as through a
/// polarizer, and the metal all of its; the lamps' lighting gives it back.
@MainActor
@Test func theShowcaseTakesHalfTheEnamelsGlare() async throws {
    let here = URL(fileURLWithPath: #filePath)
    let url = here.deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().appendingPathComponent("fixtures/svg/sample-badge.svg")
    let badge = try await LusterEngine.mint(.url(url))
    let scene = try await LusterScene.make(badge, appearance: LusterAppearance(lighting: .showcase))

    func glare() -> (enamel: Set<Float>, metal: Set<Float>) {
        let materials = (scene.model.model?.materials ?? []).compactMap { $0 as? PhysicallyBasedMaterial }
        let enamel = materials.filter { $0.metallic.scale == 0 }.map(\.specular.scale)
        let metal = materials.filter { $0.metallic.scale == 1 }.map(\.specular.scale)
        return (Set(enamel), Set(metal))
    }

    let showcase = glare()
    #expect(!showcase.enamel.isEmpty && !showcase.metal.isEmpty)
    #expect(showcase.enamel == [0.25])
    #expect(showcase.metal == [0.5])

    await scene.apply(LusterAppearance(lighting: .off))
    #expect(glare().enamel == [0.5])
}
