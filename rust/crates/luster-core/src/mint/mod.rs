//! Striking a badge: turning read artwork into the pieces of a badge.
//!
//! A badge is its own silhouette in metal, with the document's colours set
//! into it as enamel. The metal reaches a margin past the art, and its front
//! edge is rolled.

pub mod badge;
pub mod consts;
pub mod cloisonne;
pub mod plate;

pub use badge::{Piece, Struck, strike};
