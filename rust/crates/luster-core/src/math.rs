//! The engine's transcendental functions.
//!
//! `f32::sin` and the rest call the platform's own maths library, which is
//! free to round differently on Apple, bionic and glibc. A badge has to come
//! out the same bytes everywhere, so everything here goes through `libm`, one
//! implementation compiled the same on every target. `sqrt` and the basic
//! operations are exactly rounded by IEEE 754 and need no help.
//!
//! clippy.toml forbids the std versions, so a new call cannot slip back in.

/// A float the functions below work on.
pub trait Real: Copy {
    fn sin(self) -> Self;
    fn cos(self) -> Self;
    fn tan(self) -> Self;
    fn acos(self) -> Self;
    fn atan(self) -> Self;
    fn atan2(self, x: Self) -> Self;
    fn exp(self) -> Self;
    fn log2(self) -> Self;
    fn powf(self, n: Self) -> Self;
    fn hypot(self, y: Self) -> Self;
}

impl Real for f32 {
    fn sin(self) -> f32 { libm::sinf(self) }
    fn cos(self) -> f32 { libm::cosf(self) }
    fn tan(self) -> f32 { libm::tanf(self) }
    fn acos(self) -> f32 { libm::acosf(self) }
    fn atan(self) -> f32 { libm::atanf(self) }
    fn atan2(self, x: f32) -> f32 { libm::atan2f(self, x) }
    fn exp(self) -> f32 { libm::expf(self) }
    fn log2(self) -> f32 { libm::log2f(self) }
    fn powf(self, n: f32) -> f32 { libm::powf(self, n) }
    fn hypot(self, y: f32) -> f32 { libm::hypotf(self, y) }
}

impl Real for f64 {
    fn sin(self) -> f64 { libm::sin(self) }
    fn cos(self) -> f64 { libm::cos(self) }
    fn tan(self) -> f64 { libm::tan(self) }
    fn acos(self) -> f64 { libm::acos(self) }
    fn atan(self) -> f64 { libm::atan(self) }
    fn atan2(self, x: f64) -> f64 { libm::atan2(self, x) }
    fn exp(self) -> f64 { libm::exp(self) }
    fn log2(self) -> f64 { libm::log2(self) }
    fn powf(self, n: f64) -> f64 { libm::pow(self, n) }
    fn hypot(self, y: f64) -> f64 { libm::hypot(self, y) }
}

pub fn sin<T: Real>(x: T) -> T { x.sin() }
pub fn cos<T: Real>(x: T) -> T { x.cos() }
pub fn tan<T: Real>(x: T) -> T { x.tan() }
pub fn acos<T: Real>(x: T) -> T { x.acos() }
pub fn atan<T: Real>(x: T) -> T { x.atan() }
/// The angle of (x, y), as `y.atan2(x)` is.
pub fn atan2<T: Real>(y: T, x: T) -> T { y.atan2(x) }
pub fn exp<T: Real>(x: T) -> T { x.exp() }
pub fn log2<T: Real>(x: T) -> T { x.log2() }
pub fn powf<T: Real>(x: T, n: T) -> T { x.powf(n) }
pub fn hypot<T: Real>(x: T, y: T) -> T { x.hypot(y) }

/// A vector's length.
pub fn length(v: kurbo::Vec2) -> f64 {
    libm::hypot(v.x, v.y)
}

/// How far apart two points are.
pub fn distance(a: kurbo::Point, b: kurbo::Point) -> f64 {
    length(a - b)
}
