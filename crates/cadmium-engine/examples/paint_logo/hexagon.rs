//! The shapes the logo is built from: a pointy-top hexagon and its convex pieces.

use std::f64::consts::PI;

use crate::hand::Point;

#[derive(Clone, Copy, PartialEq)]
pub enum Side {
    Left,
    Right,
}

#[derive(Clone, Copy)]
pub struct Hexagon {
    pub center: Point,
    pub radius: f64,
}

impl Hexagon {
    /// Top first, then clockwise on screen.
    pub fn corners(&self) -> [Point; 6] {
        std::array::from_fn(|i| {
            let angle = -PI / 2.0 + i as f64 * PI / 3.0;
            (
                self.center.0 + self.radius * angle.cos(),
                self.center.1 + self.radius * angle.sin(),
            )
        })
    }

    pub fn whole(&self) -> Polygon {
        Polygon::new(self.corners().to_vec())
    }

    /// The band of the slab under this hexagon that shows below it, on one side, reaching
    /// `under` px up beneath the hexagon so no canvas peeks through the seam.
    pub fn slab_side(&self, side: Side, depth: f64, under: f64) -> Polygon {
        let c = self.corners();
        let outer = if side == Side::Left { c[4] } else { c[2] };
        let (top_outer, top_bottom) = ((outer.0, outer.1 - under), (c[3].0, c[3].1 - under));
        let (low_outer, low_bottom) = ((outer.0, outer.1 + depth), (c[3].0, c[3].1 + depth));
        Polygon::new(vec![top_outer, top_bottom, low_bottom, low_outer])
    }

    /// One side of the line from the top corner to the bottom one.
    pub fn half(&self, side: Side) -> Polygon {
        let c = self.corners();
        let corners = match side {
            Side::Left => vec![c[0], c[3], c[4], c[5]],
            Side::Right => vec![c[0], c[1], c[2], c[3]],
        };
        Polygon::new(corners)
    }
}

/// A convex polygon, with distances measured inward from its edges.
#[derive(Clone)]
pub struct Polygon {
    corners: Vec<Point>,
    centroid: Point,
}

impl Polygon {
    pub fn new(corners: Vec<Point>) -> Polygon {
        let count = corners.len() as f64;
        let centroid = corners.iter().fold((0.0, 0.0), |sum, p| {
            (sum.0 + p.0 / count, sum.1 + p.1 / count)
        });
        Polygon { corners, centroid }
    }

    pub fn edges(&self) -> Vec<(Point, Point)> {
        let n = self.corners.len();
        (0..n)
            .map(|i| (self.corners[i], self.corners[(i + 1) % n]))
            .collect()
    }

    /// The edge's unit normal, pointing into the polygon.
    pub fn inward(&self, (a, b): (Point, Point)) -> Point {
        let length = (b.0 - a.0).hypot(b.1 - a.1);
        let normal = (-(b.1 - a.1) / length, (b.0 - a.0) / length);
        let towards = (self.centroid.0 - a.0, self.centroid.1 - a.1);
        if normal.0 * towards.0 + normal.1 * towards.1 >= 0.0 {
            normal
        } else {
            (-normal.0, -normal.1)
        }
    }

    /// Distance from `p` to the nearest edge, negative outside.
    pub fn depth(&self, p: Point) -> f64 {
        self.edges()
            .into_iter()
            .map(|edge| {
                let n = self.inward(edge);
                (p.0 - edge.0.0) * n.0 + (p.1 - edge.0.1) * n.1
            })
            .fold(f64::INFINITY, f64::min)
    }

    /// Lowest and highest projection of the corners on the direction `angle`.
    pub fn extent(&self, angle: f64) -> (f64, f64) {
        let (dx, dy) = (angle.cos(), angle.sin());
        self.corners
            .iter()
            .map(|p| p.0 * dx + p.1 * dy)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                (lo.min(v), hi.max(v))
            })
    }

    pub fn top_and_bottom(&self) -> (f64, f64) {
        self.extent(PI / 2.0)
    }
}
