//! Small GeoJSON helpers: making features and geometries, and reading coordinates.
//!
//! Coordinates are `[longitude, latitude]`, as Litus sends and takes them.

use serde_json::{Value, json};

/// A Feature with `geometry` (or `Value::Null` for none) and `properties`.
pub fn feature(geometry: Value, properties: Value) -> Value {
    json!({"type": "Feature", "geometry": geometry, "properties": properties})
}

/// A FeatureCollection of `features`.
pub fn collection(features: Vec<Value>) -> Value {
    json!({"type": "FeatureCollection", "features": features})
}

pub fn point(p: [f64; 2]) -> Value {
    json!({"type": "Point", "coordinates": p})
}

pub fn line_string(points: &[[f64; 2]]) -> Value {
    json!({"type": "LineString", "coordinates": points})
}

/// A Polygon with one ring, closed if it isn't already.
pub fn polygon(ring: &[[f64; 2]]) -> Value {
    json!({"type": "Polygon", "coordinates": [closed(ring)]})
}

/// A Polygon with an outer ring and holes, each closed if it isn't already.
pub fn polygon_with_holes(rings: &[Vec<[f64; 2]>]) -> Value {
    json!({"type": "Polygon", "coordinates": rings.iter().map(|r| closed(r)).collect::<Vec<_>>()})
}

fn closed(ring: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut r = ring.to_vec();
    if let (Some(first), Some(last)) = (r.first().copied(), r.last())
        && first != *last
    {
        r.push(first);
    }
    r
}

/// Every position in a geometry (any type, `GeometryCollection` included), in order.
pub fn positions(geometry: &Value) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    if let Some(geoms) = geometry.get("geometries").and_then(Value::as_array) {
        for g in geoms {
            out.extend(positions(g));
        }
    } else if let Some(c) = geometry.get("coordinates") {
        collect(c, &mut out);
    }
    out
}

fn collect(v: &Value, out: &mut Vec<[f64; 2]>) {
    let Some(a) = v.as_array() else { return };
    match (a.first().and_then(Value::as_f64), a.get(1).and_then(Value::as_f64)) {
        (Some(x), Some(y)) => out.push([x, y]),
        _ => a.iter().for_each(|c| collect(c, out)),
    }
}

/// The box around `points`, `[xmin, ymin, xmax, ymax]`; `None` when there are none.
pub fn bbox(points: &[[f64; 2]]) -> Option<[f64; 4]> {
    let (first, rest) = points.split_first()?;
    Some(
        rest.iter().fold([first[0], first[1], first[0], first[1]], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_of_every_type() {
        assert_eq!(positions(&point([1.0, 2.0])), [[1.0, 2.0]]);
        let poly = polygon(&[[0.0, 0.0], [2.0, 0.0], [2.0, 3.0]]);
        // The ring was closed.
        assert_eq!(positions(&poly).len(), 4);
        let gc = json!({"type": "GeometryCollection", "geometries": [point([5.0, -1.0]), poly]});
        assert_eq!(bbox(&positions(&gc)), Some([0.0, -1.0, 5.0, 3.0]));
        // A third coordinate (height) is ignored; junk is skipped.
        assert_eq!(positions(&json!({"type": "Point", "coordinates": [1, 2, 30]})), [[1.0, 2.0]]);
        assert!(positions(&json!({"coordinates": "x"})).is_empty());
        assert_eq!(bbox(&[]), None);
    }
}
