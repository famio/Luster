internal import LusterCore

/// How a badge answers a drag: the engine's numbers, so a flick feels the same
/// on every platform. For the views in LusterUI.
package struct LusterHandling: Sendable {
    /// Radians per point dragged.
    package let spinPerPoint: Float
    package let tiltPerPoint: Float
    package let tiltLimit: Float
    /// The closest a pinch brings the badge, times its size at rest.
    package let closest: Float

    package static let standard: LusterHandling = {
        let h = LusterCore.handling()
        return LusterHandling(
            spinPerPoint: h.spinPerPoint, tiltPerPoint: h.tiltPerPoint,
            tiltLimit: h.tiltLimit, closest: h.closest)
    }()

    /// Brings the badge `zoom` times closer with `point` of its frame seen at
    /// `at` in the view, as near as the frame lets it.
    package static func look(_ zoom: Float, point: SIMD2<Float>, at: SIMD2<Float>) -> LusterLook {
        let l = LusterCore.look(zoom: zoom, pointX: point.x, pointY: point.y, atX: at.x, atY: at.y)
        return LusterLook(zoom: l.zoom, x: l.x, y: l.y)
    }

    /// Carries a flick's spin (radians per second) over `dt` seconds: how far
    /// the badge turns, and the speed it is left with.
    package static func coast(_ velocity: Float, dt: Float) -> (turn: Float, velocity: Float) {
        let coast = LusterCore.coast(velocity: velocity, dt: dt)
        return (coast.turn, coast.velocity)
    }
}

/// What a view shows of the badge brought closer: `zoom` times its size at
/// rest, centred on `x`, `y`, a point of the frame it fits at rest, in
/// fractions of that frame from its middle, x to the right and y down.
package struct LusterLook: Equatable, Sendable {
    package var zoom: Float
    package var x: Float
    package var y: Float

    package static let atRest = LusterLook(zoom: 1, x: 0, y: 0)

    package init(zoom: Float, x: Float, y: Float) {
        self.zoom = zoom
        self.x = x
        self.y = y
    }

    /// The point of the frame seen at `at` in the view, in fractions of the
    /// view from its middle.
    package func under(_ at: SIMD2<Float>) -> SIMD2<Float> {
        SIMD2(x + at.x / zoom, y + at.y / zoom)
    }
}
