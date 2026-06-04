use crate::config;
use crate::game::entity::{Entity, EntityKind};
use crate::game::map::Map;
use crate::game::services::occupancy::building_footprint;

const DEFAULT_FACING_RAD: f32 = 0.0;
const CIRCLE_COLLISION_EPS_PX: f32 = 0.001;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum UnitBody {
    Circle(CircleBody),
    OrientedBox(OrientedBoxBody),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CircleBody {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OrientedBoxBody {
    pub x: f32,
    pub y: f32,
    pub half_len: f32,
    pub half_width: f32,
    pub facing: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RectBody {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BodyAabb {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BodyOverlap {
    /// Unit vector pointing from the first body toward the second body.
    pub normal_x: f32,
    pub normal_y: f32,
    pub depth: f32,
}

impl UnitBody {
    pub(crate) fn bounding_radius(self) -> f32 {
        match self {
            UnitBody::Circle(body) => body.radius,
            UnitBody::OrientedBox(body) => {
                (body.half_len * body.half_len + body.half_width * body.half_width).sqrt()
            }
        }
    }

    pub(crate) fn aabb(self) -> BodyAabb {
        match self {
            UnitBody::Circle(body) => BodyAabb {
                min_x: body.x - body.radius,
                min_y: body.y - body.radius,
                max_x: body.x + body.radius,
                max_y: body.y + body.radius,
            },
            UnitBody::OrientedBox(body) => {
                let (fx, fy) = body.forward_axis();
                let (sx, sy) = body.side_axis();
                let extent_x = fx.abs() * body.half_len + sx.abs() * body.half_width;
                let extent_y = fy.abs() * body.half_len + sy.abs() * body.half_width;
                BodyAabb {
                    min_x: body.x - extent_x,
                    min_y: body.y - extent_y,
                    max_x: body.x + extent_x,
                    max_y: body.y + extent_y,
                }
            }
        }
    }
}

impl OrientedBoxBody {
    fn forward_axis(self) -> (f32, f32) {
        (self.facing.cos(), self.facing.sin())
    }

    fn side_axis(self) -> (f32, f32) {
        let (fx, fy) = self.forward_axis();
        (-fy, fx)
    }
}

pub(crate) fn unit_body(kind: EntityKind, x: f32, y: f32) -> Option<UnitBody> {
    unit_body_with_facing(kind, x, y, DEFAULT_FACING_RAD)
}

pub(crate) fn unit_body_for_entity(e: &Entity) -> Option<UnitBody> {
    unit_body_with_facing(e.kind, e.pos_x, e.pos_y, e.facing())
}

pub(crate) fn unit_body_with_facing(
    kind: EntityKind,
    x: f32,
    y: f32,
    facing: f32,
) -> Option<UnitBody> {
    let stats = config::unit_stats(kind)?;
    if !x.is_finite() || !y.is_finite() || !stats.radius.is_finite() || stats.radius <= 0.0 {
        return None;
    }

    if kind == EntityKind::Tank {
        let clearance = config::TANK_BODY_CLEARANCE_PX;
        let half_len = config::TANK_BODY_LENGTH_PX * 0.5 + clearance;
        let half_width = config::TANK_BODY_WIDTH_PX * 0.5 + clearance;
        if !facing.is_finite()
            || !half_len.is_finite()
            || !half_width.is_finite()
            || half_len <= 0.0
            || half_width <= 0.0
        {
            return None;
        }
        return Some(UnitBody::OrientedBox(OrientedBoxBody {
            x,
            y,
            half_len,
            half_width,
            facing,
        }));
    }

    Some(UnitBody::Circle(CircleBody {
        x,
        y,
        radius: stats.radius,
    }))
}

pub(crate) fn building_rect_for_footprint(
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> Option<RectBody> {
    let stats = config::building_stats(kind)?;
    if stats.foot_w == 0 || stats.foot_h == 0 {
        return None;
    }

    let max_tile_x = tile_x.checked_add(stats.foot_w)?;
    let max_tile_y = tile_y.checked_add(stats.foot_h)?;
    let ts = config::TILE_SIZE as f32;

    Some(RectBody {
        min_x: tile_x as f32 * ts,
        min_y: tile_y as f32 * ts,
        max_x: max_tile_x as f32 * ts,
        max_y: max_tile_y as f32 * ts,
    })
}

pub(crate) fn building_rect_for_entity(map: &Map, e: &Entity) -> Option<RectBody> {
    config::building_stats(e.kind)?;
    if !e.pos_x.is_finite() || !e.pos_y.is_finite() {
        return None;
    }

    let footprint = building_footprint(map, e);
    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0;
    let mut max_y = 0;
    for (tx, ty) in footprint {
        min_x = min_x.min(tx);
        min_y = min_y.min(ty);
        max_x = max_x.max(tx.checked_add(1)?);
        max_y = max_y.max(ty.checked_add(1)?);
    }
    if min_x == u32::MAX || min_y == u32::MAX {
        return None;
    }
    let ts = config::TILE_SIZE as f32;

    Some(RectBody {
        min_x: min_x as f32 * ts,
        min_y: min_y as f32 * ts,
        max_x: max_x as f32 * ts,
        max_y: max_y as f32 * ts,
    })
}

pub(crate) fn circle_intersects_rect(circle: CircleBody, rect: RectBody) -> bool {
    if !valid_circle(circle) || !valid_rect(rect) {
        return false;
    }

    let nearest_x = circle.x.clamp(rect.min_x, rect.max_x);
    let nearest_y = circle.y.clamp(rect.min_y, rect.max_y);
    let dx = circle.x - nearest_x;
    let dy = circle.y - nearest_y;
    dx * dx + dy * dy <= circle.radius * circle.radius
}

pub(crate) fn unit_body_intersects_rect(body: UnitBody, rect: RectBody) -> bool {
    if !valid_unit_body(body) || !valid_rect(rect) {
        return false;
    }
    match body {
        UnitBody::Circle(circle) => circle_intersects_rect(circle, rect),
        UnitBody::OrientedBox(oriented) => oriented_box_intersects_rect(oriented, rect),
    }
}

pub(crate) fn unit_bodies_intersect(a: UnitBody, b: UnitBody) -> bool {
    if !valid_unit_body(a) || !valid_unit_body(b) {
        return false;
    }
    match (a, b) {
        (UnitBody::Circle(a), UnitBody::Circle(b)) => circles_intersect(a, b),
        (UnitBody::Circle(circle), UnitBody::OrientedBox(box_body))
        | (UnitBody::OrientedBox(box_body), UnitBody::Circle(circle)) => {
            circle_intersects_oriented_box(circle, box_body)
        }
        (UnitBody::OrientedBox(a), UnitBody::OrientedBox(b)) => oriented_boxes_intersect(a, b),
    }
}

pub(crate) fn unit_body_overlap(a: UnitBody, b: UnitBody) -> Option<BodyOverlap> {
    if !valid_unit_body(a) || !valid_unit_body(b) {
        return None;
    }

    let overlap = match (a, b) {
        (UnitBody::Circle(a), UnitBody::Circle(b)) => circle_circle_overlap(a, b),
        (UnitBody::Circle(circle), UnitBody::OrientedBox(box_body)) => {
            circle_oriented_box_overlap(circle, box_body)
        }
        (UnitBody::OrientedBox(box_body), UnitBody::Circle(circle)) => {
            circle_oriented_box_overlap(circle, box_body).map(|overlap| BodyOverlap {
                normal_x: -overlap.normal_x,
                normal_y: -overlap.normal_y,
                depth: overlap.depth,
            })
        }
        (UnitBody::OrientedBox(a), UnitBody::OrientedBox(b)) => oriented_box_overlap(a, b),
    }?;

    (overlap.depth.is_finite() && overlap.depth > 0.0).then_some(overlap)
}

pub(crate) fn tile_rect(tx: i32, ty: i32) -> RectBody {
    let ts = config::TILE_SIZE as f32;
    RectBody {
        min_x: tx as f32 * ts,
        min_y: ty as f32 * ts,
        max_x: (tx + 1) as f32 * ts,
        max_y: (ty + 1) as f32 * ts,
    }
}

pub(crate) fn rects_intersect(a: RectBody, b: RectBody) -> bool {
    valid_rect(a)
        && valid_rect(b)
        && a.min_x < b.max_x
        && a.max_x > b.min_x
        && a.min_y < b.max_y
        && a.max_y > b.min_y
}

pub(crate) fn segment_intersects_unit_body(
    start: (f32, f32),
    end: (f32, f32),
    body: UnitBody,
) -> Option<f32> {
    if !valid_segment(start, end) || !valid_unit_body(body) {
        return None;
    }
    match body {
        UnitBody::Circle(circle) => segment_intersects_circle(start, end, circle),
        UnitBody::OrientedBox(body) => segment_intersects_oriented_box(start, end, body),
    }
}

pub(crate) fn segment_intersects_rect(
    start: (f32, f32),
    end: (f32, f32),
    rect: RectBody,
) -> Option<f32> {
    if !valid_segment(start, end) || !valid_rect(rect) {
        return None;
    }
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let mut t_min = 0.0f32;
    let mut t_max = 1.0f32;

    if !clip_segment_axis(start.0, dx, rect.min_x, rect.max_x, &mut t_min, &mut t_max) {
        return None;
    }
    if !clip_segment_axis(start.1, dy, rect.min_y, rect.max_y, &mut t_min, &mut t_max) {
        return None;
    }
    Some(t_min.clamp(0.0, 1.0))
}

fn oriented_box_intersects_rect(body: OrientedBoxBody, rect: RectBody) -> bool {
    let rect_center = (
        (rect.min_x + rect.max_x) * 0.5,
        (rect.min_y + rect.max_y) * 0.5,
    );
    let rect_half = (
        (rect.max_x - rect.min_x) * 0.5,
        (rect.max_y - rect.min_y) * 0.5,
    );
    let (fx, fy) = body.forward_axis();
    let (sx, sy) = body.side_axis();
    let dx = rect_center.0 - body.x;
    let dy = rect_center.1 - body.y;

    let axes = [(fx, fy), (sx, sy), (1.0, 0.0), (0.0, 1.0)];
    axes.into_iter().all(|axis| {
        let center_dist = (dx * axis.0 + dy * axis.1).abs();
        let body_extent =
            body.half_len * dot_abs(axis, (fx, fy)) + body.half_width * dot_abs(axis, (sx, sy));
        let rect_extent = rect_half.0 * axis.0.abs() + rect_half.1 * axis.1.abs();
        center_dist <= body_extent + rect_extent
    })
}

fn circle_intersects_oriented_box(circle: CircleBody, body: OrientedBoxBody) -> bool {
    let (fx, fy) = body.forward_axis();
    let (sx, sy) = body.side_axis();
    let dx = circle.x - body.x;
    let dy = circle.y - body.y;
    let local_x = (dx * fx + dy * fy).clamp(-body.half_len, body.half_len);
    let local_y = (dx * sx + dy * sy).clamp(-body.half_width, body.half_width);
    let closest_x = body.x + fx * local_x + sx * local_y;
    let closest_y = body.y + fy * local_x + sy * local_y;
    let px = circle.x - closest_x;
    let py = circle.y - closest_y;
    px * px + py * py <= circle.radius * circle.radius
}

fn oriented_boxes_intersect(a: OrientedBoxBody, b: OrientedBoxBody) -> bool {
    let (afx, afy) = a.forward_axis();
    let (asx, asy) = a.side_axis();
    let (bfx, bfy) = b.forward_axis();
    let (bsx, bsy) = b.side_axis();
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let axes = [(afx, afy), (asx, asy), (bfx, bfy), (bsx, bsy)];

    axes.into_iter().all(|axis| {
        let center_dist = (dx * axis.0 + dy * axis.1).abs();
        let a_extent =
            a.half_len * dot_abs(axis, (afx, afy)) + a.half_width * dot_abs(axis, (asx, asy));
        let b_extent =
            b.half_len * dot_abs(axis, (bfx, bfy)) + b.half_width * dot_abs(axis, (bsx, bsy));
        center_dist <= a_extent + b_extent
    })
}

fn circle_circle_overlap(a: CircleBody, b: CircleBody) -> Option<BodyOverlap> {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let min_d = a.radius + b.radius;
    let d2 = dx * dx + dy * dy;
    if d2 + CIRCLE_COLLISION_EPS_PX >= min_d * min_d {
        return None;
    }

    if d2 <= 1.0e-4 {
        return Some(BodyOverlap {
            normal_x: 1.0,
            normal_y: 0.0,
            depth: min_d,
        });
    }

    let d = d2.sqrt();
    Some(BodyOverlap {
        normal_x: dx / d,
        normal_y: dy / d,
        depth: min_d - d,
    })
}

fn circle_oriented_box_overlap(circle: CircleBody, body: OrientedBoxBody) -> Option<BodyOverlap> {
    let (fx, fy) = body.forward_axis();
    let (sx, sy) = body.side_axis();
    let dx = circle.x - body.x;
    let dy = circle.y - body.y;
    let local_x = dx * fx + dy * fy;
    let local_y = dx * sx + dy * sy;
    let clamped_x = local_x.clamp(-body.half_len, body.half_len);
    let clamped_y = local_y.clamp(-body.half_width, body.half_width);
    let closest_x = body.x + fx * clamped_x + sx * clamped_y;
    let closest_y = body.y + fy * clamped_x + sy * clamped_y;
    let from_box_x = circle.x - closest_x;
    let from_box_y = circle.y - closest_y;
    let d2 = from_box_x * from_box_x + from_box_y * from_box_y;

    if d2 > 1.0e-4 {
        if d2 + 0.000001 >= circle.radius * circle.radius {
            return None;
        }
        let d = d2.sqrt();
        return Some(BodyOverlap {
            normal_x: -from_box_x / d,
            normal_y: -from_box_y / d,
            depth: circle.radius - d,
        });
    }

    let x_depth = body.half_len - local_x.abs() + circle.radius;
    let y_depth = body.half_width - local_y.abs() + circle.radius;
    if x_depth <= y_depth {
        let sign = if local_x >= 0.0 { -1.0 } else { 1.0 };
        Some(BodyOverlap {
            normal_x: fx * sign,
            normal_y: fy * sign,
            depth: x_depth,
        })
    } else {
        let sign = if local_y >= 0.0 { -1.0 } else { 1.0 };
        Some(BodyOverlap {
            normal_x: sx * sign,
            normal_y: sy * sign,
            depth: y_depth,
        })
    }
}

fn oriented_box_overlap(a: OrientedBoxBody, b: OrientedBoxBody) -> Option<BodyOverlap> {
    let (afx, afy) = a.forward_axis();
    let (asx, asy) = a.side_axis();
    let (bfx, bfy) = b.forward_axis();
    let (bsx, bsy) = b.side_axis();
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let axes = [(afx, afy), (asx, asy), (bfx, bfy), (bsx, bsy)];

    let mut best_axis = (1.0, 0.0);
    let mut best_depth = f32::INFINITY;
    for axis in axes {
        let center_delta = dx * axis.0 + dy * axis.1;
        let center_dist = center_delta.abs();
        let a_extent =
            a.half_len * dot_abs(axis, (afx, afy)) + a.half_width * dot_abs(axis, (asx, asy));
        let b_extent =
            b.half_len * dot_abs(axis, (bfx, bfy)) + b.half_width * dot_abs(axis, (bsx, bsy));
        let depth = a_extent + b_extent - center_dist;
        if depth <= 0.0 {
            return None;
        }
        if depth < best_depth {
            let sign = if center_delta >= 0.0 { 1.0 } else { -1.0 };
            best_axis = (axis.0 * sign, axis.1 * sign);
            best_depth = depth;
        }
    }

    Some(BodyOverlap {
        normal_x: best_axis.0,
        normal_y: best_axis.1,
        depth: best_depth,
    })
}

fn circles_intersect(a: CircleBody, b: CircleBody) -> bool {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let r = a.radius + b.radius;
    dx * dx + dy * dy <= r * r
}

fn segment_intersects_circle(
    start: (f32, f32),
    end: (f32, f32),
    circle: CircleBody,
) -> Option<f32> {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let fx = start.0 - circle.x;
    let fy = start.1 - circle.y;
    let a = dx * dx + dy * dy;
    if a <= f32::EPSILON {
        return None;
    }
    let b = 2.0 * (fx * dx + fy * dy);
    let c = fx * fx + fy * fy - circle.radius * circle.radius;
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }
    let sqrt = discriminant.sqrt();
    let t1 = (-b - sqrt) / (2.0 * a);
    let t2 = (-b + sqrt) / (2.0 * a);
    [t1, t2]
        .into_iter()
        .filter(|t| (0.0..=1.0).contains(t))
        .min_by(|a, b| a.total_cmp(b))
}

fn segment_intersects_oriented_box(
    start: (f32, f32),
    end: (f32, f32),
    body: OrientedBoxBody,
) -> Option<f32> {
    let (fx, fy) = body.forward_axis();
    let (sx, sy) = body.side_axis();
    let local_start = (
        (start.0 - body.x) * fx + (start.1 - body.y) * fy,
        (start.0 - body.x) * sx + (start.1 - body.y) * sy,
    );
    let local_end = (
        (end.0 - body.x) * fx + (end.1 - body.y) * fy,
        (end.0 - body.x) * sx + (end.1 - body.y) * sy,
    );
    segment_intersects_rect(
        local_start,
        local_end,
        RectBody {
            min_x: -body.half_len,
            min_y: -body.half_width,
            max_x: body.half_len,
            max_y: body.half_width,
        },
    )
}

fn clip_segment_axis(
    origin: f32,
    delta: f32,
    min: f32,
    max: f32,
    t_min: &mut f32,
    t_max: &mut f32,
) -> bool {
    if delta.abs() <= f32::EPSILON {
        return origin >= min && origin <= max;
    }
    let inv = 1.0 / delta;
    let mut near = (min - origin) * inv;
    let mut far = (max - origin) * inv;
    if near > far {
        std::mem::swap(&mut near, &mut far);
    }
    *t_min = (*t_min).max(near);
    *t_max = (*t_max).min(far);
    *t_min <= *t_max
}

fn dot_abs(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 * b.0 + a.1 * b.1).abs()
}

fn valid_unit_body(body: UnitBody) -> bool {
    match body {
        UnitBody::Circle(circle) => valid_circle(circle),
        UnitBody::OrientedBox(body) => valid_oriented_box(body),
    }
}

fn valid_circle(circle: CircleBody) -> bool {
    circle.x.is_finite()
        && circle.y.is_finite()
        && circle.radius.is_finite()
        && circle.radius >= 0.0
}

fn valid_oriented_box(body: OrientedBoxBody) -> bool {
    body.x.is_finite()
        && body.y.is_finite()
        && body.half_len.is_finite()
        && body.half_width.is_finite()
        && body.facing.is_finite()
        && body.half_len >= 0.0
        && body.half_width >= 0.0
}

fn valid_rect(rect: RectBody) -> bool {
    rect.min_x.is_finite()
        && rect.min_y.is_finite()
        && rect.max_x.is_finite()
        && rect.max_y.is_finite()
        && rect.min_x <= rect.max_x
        && rect.min_y <= rect.max_y
}

fn valid_segment(start: (f32, f32), end: (f32, f32)) -> bool {
    start.0.is_finite()
        && start.1.is_finite()
        && end.0.is_finite()
        && end.1.is_finite()
        && ((end.0 - start.0).abs() > f32::EPSILON || (end.1 - start.1).abs() > f32::EPSILON)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::services::occupancy::footprint_center;

    #[test]
    fn tank_body_intersects_building_even_when_center_tile_is_clear() {
        let rect = building_rect_for_footprint(EntityKind::Depot, 4, 4).expect("depot rect");
        let tank = unit_body_with_facing(
            EntityKind::Tank,
            rect.max_x + config::TANK_BODY_WIDTH_PX * 0.5,
            rect.min_y + 32.0,
            std::f32::consts::FRAC_PI_2,
        )
        .expect("tank body");
        let UnitBody::OrientedBox(tank_box) = tank else {
            panic!("tank should use oriented body");
        };

        assert_eq!((tank_box.x / config::TILE_SIZE as f32).floor() as u32, 6);
        assert!(unit_body_intersects_rect(tank, rect));
    }

    #[test]
    fn tank_front_clearance_uses_hull_length() {
        let rect = building_rect_for_footprint(EntityKind::Depot, 4, 4).expect("depot rect");
        let legal = unit_body_with_facing(
            EntityKind::Tank,
            rect.min_x - (config::TANK_BODY_LENGTH_PX * 0.5 + config::TANK_BODY_CLEARANCE_PX) - 0.1,
            rect.min_y + 32.0,
            0.0,
        )
        .expect("tank body");
        let illegal = unit_body_with_facing(
            EntityKind::Tank,
            rect.min_x - (config::TANK_BODY_LENGTH_PX * 0.5 + config::TANK_BODY_CLEARANCE_PX) + 0.1,
            rect.min_y + 32.0,
            0.0,
        )
        .expect("tank body");

        assert!(!unit_body_intersects_rect(legal, rect));
        assert!(unit_body_intersects_rect(illegal, rect));
    }

    #[test]
    fn tank_side_clearance_uses_hull_width() {
        let rect = building_rect_for_footprint(EntityKind::Depot, 4, 4).expect("depot rect");
        let legal = unit_body_with_facing(
            EntityKind::Tank,
            rect.min_x + 32.0,
            rect.max_y + (config::TANK_BODY_WIDTH_PX * 0.5 + config::TANK_BODY_CLEARANCE_PX) + 0.1,
            0.0,
        )
        .expect("tank body");
        let illegal = unit_body_with_facing(
            EntityKind::Tank,
            rect.min_x + 32.0,
            rect.max_y + (config::TANK_BODY_WIDTH_PX * 0.5 + config::TANK_BODY_CLEARANCE_PX) - 0.1,
            0.0,
        )
        .expect("tank body");

        assert!(!unit_body_intersects_rect(legal, rect));
        assert!(unit_body_intersects_rect(illegal, rect));
    }

    #[test]
    fn building_rect_for_entity_matches_centered_footprint_tiles() {
        let map = flat_map(12);
        let (x, y) = footprint_center(&map, EntityKind::Depot, 4, 4);
        let building =
            Entity::new_building(1, EntityKind::Depot, x, y, true).expect("depot should spawn");

        assert_eq!(
            building_rect_for_entity(&map, &building),
            building_rect_for_footprint(EntityKind::Depot, 4, 4)
        );
    }

    #[test]
    fn segment_intersection_reports_first_rect_contact() {
        let rect = RectBody {
            min_x: 20.0,
            min_y: 5.0,
            max_x: 40.0,
            max_y: 15.0,
        };

        let hit = segment_intersects_rect((0.0, 10.0), (100.0, 10.0), rect)
            .expect("segment should hit rect");

        assert!((hit - 0.2).abs() < 0.001);
    }

    #[test]
    fn segment_intersection_uses_oriented_tank_body() {
        let body = unit_body_with_facing(EntityKind::Tank, 50.0, 50.0, std::f32::consts::FRAC_PI_2)
            .expect("tank body");

        assert!(
            segment_intersects_unit_body((50.0, 0.0), (50.0, 100.0), body).is_some(),
            "lengthwise segment should hit the oriented hull"
        );
        assert!(
            segment_intersects_unit_body((0.0, 20.0), (100.0, 20.0), body).is_none(),
            "segment outside the oriented hull should miss"
        );
    }

    fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![crate::protocol::terrain::GRASS; (size * size) as usize],
            starts: vec![],
            expansion_sites: vec![],
        }
    }
}
