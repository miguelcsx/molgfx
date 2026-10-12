//! Discrete beta boundaries and consistently oriented frames, in O(guides).

use molgfx_math::Vec3;

#[derive(Clone, Copy, Debug)]
pub(super) struct StrandFrame {
    pub center: Vec3,
    pub width: Vec3,
    pub height: Vec3,
}

pub(super) fn frames(points: &[Vec3]) -> Vec<StrandFrame> {
    if points.len() < 3 {
        return Vec::new();
    }
    let mut output = Vec::with_capacity(points.len() + 1);
    for boundary in 0..=points.len() {
        let center = if boundary == 0 {
            points[0] + points[1] * 0.5 - points[2] * 0.5
        } else if boundary == points.len() {
            points[boundary - 1] + points[boundary - 2] * 0.5 - points[boundary - 3] * 0.5
        } else {
            (points[boundary - 1] + points[boundary]) * 0.5
        };
        let guide = boundary.clamp(1, points.len() - 2);
        let left = points[guide] - points[guide - 1];
        let right = points[guide + 1] - points[guide];
        let width = unit(left.cross(right), perpendicular(right))
            * if guide.is_multiple_of(2) { 1.0 } else { -1.0 };
        output.push(StrandFrame {
            center,
            width,
            height: Vec3::ZERO,
        });
    }
    for boundary in 0..points.len() {
        let span = output[boundary].center - output[boundary + 1].center;
        output[boundary].height = unit(
            span.cross(output[boundary].width),
            perpendicular(output[boundary].width),
        );
    }
    output[points.len()].height = output[points.len() - 1].height;
    for boundary in 1..points.len() {
        if output[boundary - 1].height.dot(output[boundary].height) < 0.0 {
            output[boundary].width = -output[boundary].width;
            output[boundary].height = -output[boundary].height;
        }
    }
    // The tip shares the last guide's unsplit orientation.
    output[points.len()] = StrandFrame {
        center: output[points.len()].center,
        ..output[points.len() - 1]
    };
    output
}

pub(super) fn sample(frames: &[StrandFrame], interval: usize, parameter: f32) -> StrandFrame {
    let left = frames[interval];
    let right = frames[interval + 1];
    if parameter.to_bits() == 0 {
        return left;
    }
    if parameter.to_bits() == 1.0f32.to_bits() {
        return right;
    }
    if interval == 0 || interval + 2 == frames.len() || left.height.dot(right.height).abs() > 0.9 {
        return StrandFrame {
            center: left.center.lerp(right.center, parameter),
            width: unit(left.width.lerp(right.width, parameter), left.width),
            height: unit(left.height.lerp(right.height, parameter), left.height),
        };
    }
    let previous = frames[interval - 1];
    let next = frames[interval + 2];
    StrandFrame {
        center: catmull(
            previous.center,
            left.center,
            right.center,
            next.center,
            parameter,
        ),
        width: unit(
            catmull(
                previous.width,
                left.width,
                right.width,
                next.width,
                parameter,
            ),
            left.width,
        ),
        height: unit(
            catmull(
                previous.height,
                left.height,
                right.height,
                next.height,
                parameter,
            ),
            left.height,
        ),
    }
}

fn catmull(previous: Vec3, left: Vec3, right: Vec3, next: Vec3, parameter: f32) -> Vec3 {
    (left * 2.0
        + (right - previous) * parameter
        + (previous * 2.0 - left * 5.0 + right * 4.0 - next) * parameter * parameter
        + (-previous + left * 3.0 - right * 3.0 + next) * parameter * parameter * parameter)
        * 0.5
}

fn unit(value: Vec3, fallback: Vec3) -> Vec3 {
    if value.length_squared() < 1e-12 {
        fallback
    } else {
        value.normalize()
    }
}

fn perpendicular(value: Vec3) -> Vec3 {
    let axis = if value.x.abs() <= value.y.abs() && value.x.abs() <= value.z.abs() {
        Vec3::X
    } else if value.y.abs() <= value.z.abs() {
        Vec3::Y
    } else {
        Vec3::Z
    };
    unit(value.cross(axis), Vec3::Z)
}

#[cfg(test)]
#[path = "strand_frames_tests.rs"]
mod tests;
