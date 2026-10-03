internal import LusterCore

/// How a badge answers a drag: the engine's numbers, so a flick feels the same
/// on every platform. For the views in LusterUI.
package struct LusterHandling: Sendable {
    /// Radians per point dragged.
    package let spinPerPoint: Float
    package let tiltPerPoint: Float
    package let tiltLimit: Float

    package static let standard: LusterHandling = {
        let h = LusterCore.handling()
        return LusterHandling(spinPerPoint: h.spinPerPoint, tiltPerPoint: h.tiltPerPoint,
                              tiltLimit: h.tiltLimit)
    }()

    /// Carries a flick's spin (radians per second) over `dt` seconds: how far
    /// the badge turns, and the speed it is left with.
    package static func coast(_ velocity: Float, dt: Float) -> (turn: Float, velocity: Float) {
        let coast = LusterCore.coast(velocity: velocity, dt: dt)
        return (coast.turn, coast.velocity)
    }
}
