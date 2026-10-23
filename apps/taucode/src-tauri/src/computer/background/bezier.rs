// Copyright (c) 2025 Cua AI, Inc.
// SPDX-License-Identifier: MIT
// Adapted from trycua/cua; see NOTICE for source and license.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

pub struct CubicBezier {
    pub start: Point,
    pub control1: Point,
    pub control2: Point,
    pub end: Point,
}
impl CubicBezier {
    /// Point on the curve at `t ∈ [0,1]`.
    pub fn point_at(&self, t: f64) -> Point {
        let t = t.clamp(0.0, 1.0);
        let u = 1.0 - t;
        let (uu, uuu) = (u * u, u * u * u);
        let (tt, ttt) = (t * t, t * t * t);
        Point {
            x: uuu * self.start.x
                + 3.0 * uu * t * self.control1.x
                + 3.0 * u * tt * self.control2.x
                + ttt * self.end.x,
            y: uuu * self.start.y
                + 3.0 * uu * t * self.control1.y
                + 3.0 * u * tt * self.control2.y
                + ttt * self.end.y,
        }
    }
}
