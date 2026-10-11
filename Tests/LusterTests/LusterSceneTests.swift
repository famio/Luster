import CoreGraphics
import Foundation
import RealityKit
import Testing
@testable import Luster

/// A badge's materials are copies of finishes and colours kept for the
/// process: whichever way they are come by, made afresh, copied from what an
/// earlier badge kept, or re-plated and plated back, they come out the same.
@MainActor
@Test func materialsComeOutTheSameHoweverTheyAreMade() async throws {
    // Painted faces on a sheet, flat colours, and the reverse's grit.
    let badge = try await LusterEngine.mint(.url(fixture("many-colour-cells.svg")))
    let first = try await LusterScene.make(badge, appearance: .init())
    let again = try await LusterScene.make(badge, appearance: .init())
    let made = finishes(first)
    #expect(made.contains { $0.textured } && made.contains { $0.grit } && made.contains { !$0.textured })
    #expect(made == finishes(again))

    await first.apply(.init(metal: .silver, lighting: .off))
    #expect(made != finishes(first))
    await first.apply(.init())
    #expect(made == finishes(first))
}

/// Re-plating changes the metal and nothing else: the enamel keeps its
/// colours and every material its finish.
@MainActor
@Test func onlyTheMetalTakesTheColourItIsPlatedIn() async throws {
    let badge = try await LusterEngine.mint(.url(fixture("sample-badge.svg")))
    let gold = finishes(try await LusterScene.make(badge, appearance: .init(metal: .gold)))
    let silver = finishes(try await LusterScene.make(badge, appearance: .init(metal: .silver)))
    #expect(gold.count == silver.count)
    var plated = 0
    for (a, b) in zip(gold, silver) where a != b {
        plated += 1
        #expect(a.tint == components(.gold) && b.tint == components(.silver))
        #expect(a.metallic == b.metallic && a.roughness == b.roughness && a.specular == b.specular)
    }
    #expect(plated > 0)
}

/// What is kept is made once for everyone who asks while it is being made,
/// and a failure is tried again by the next to ask.
@MainActor
@Test func whatIsKeptIsMadeOnceAndTriedAgainAfterAFailure() async throws {
    struct Failed: Error {}
    final class Count { var made = 0; var failing = true }
    let count = Count()
    let kept = Kept<Int> {
        count.made += 1
        try await Task.sleep(for: .milliseconds(20))
        if count.failing { throw Failed() }
        return count.made
    }
    async let a = try? kept.value()
    async let b = try? kept.value()
    let together = await [a, b]
    #expect(together == [nil, nil])
    #expect(count.made == 1)
    count.failing = false
    #expect(try await kept.value() == 2)
    #expect(try await kept.value() == 2)
    #expect(count.made == 2)
}

/// What a material is made of, as far as these tests look.
private struct Made: Equatable {
    var metallic, roughness, specular: Float
    var tint: [Float]
    var textured, grit: Bool
}

@MainActor
private func finishes(_ scene: LusterScene) -> [Made] {
    (scene.model.model?.materials ?? []).compactMap { $0 as? PhysicallyBasedMaterial }.map { pbr in
        Made(metallic: pbr.metallic.scale, roughness: pbr.roughness.scale,
             specular: pbr.specular.scale, tint: components(pbr.baseColor.tint.cgColor),
             textured: pbr.baseColor.texture != nil, grit: pbr.normal.texture != nil)
    }
}

/// sRGB components, to three places: what two ways of writing one colour
/// agree on.
private func components(_ color: CGColor) -> [Float] {
    let srgb = CGColorSpace(name: CGColorSpace.sRGB)!
    let converted = color.converted(to: srgb, intent: .defaultIntent, options: nil) ?? color
    return (converted.components ?? []).prefix(3).map { (Float($0) * 1000).rounded() / 1000 }
}

private func components(_ color: LusterColor) -> [Float] {
    components(color.cgColor)
}

private func fixture(_ name: String) -> URL {
    URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().appendingPathComponent("fixtures/svg/\(name)")
}
