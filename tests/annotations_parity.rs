use eframe::egui::{Pos2, pos2, vec2};
use quicktool_rust::{
    annotations::{Annotation, Layer, Tool, encode_psd, number_color},
    geometry,
};
use serde_json::Value;
fn number(v: &Value, k: &str) -> f32 {
    v[k].as_f64().unwrap() as f32
}
fn point(v: &Value) -> Pos2 {
    pos2(number(v, "x"), number(v, "y"))
}
fn color(v: &Value) -> [u8; 3] {
    let s = v.as_str().unwrap();
    [
        u8::from_str_radix(&s[1..3], 16).unwrap(),
        u8::from_str_radix(&s[3..5], 16).unwrap(),
        u8::from_str_radix(&s[5..7], 16).unwrap(),
    ]
}
fn annotation(v: &Value) -> Annotation {
    let tool = serde_json::from_value::<Tool>(v["type"].clone()).unwrap();
    let mut a = Annotation::new(
        v["id"].as_str().unwrap().parse().unwrap(),
        tool,
        point(v),
        100.,
    );
    a.x2 = number(v, "x2");
    a.y2 = number(v, "y2");
    a.color = color(&v["color"]);
    a.radius = number(v, "radius");
    a.opacity = number(v, "opacity");
    a.stroke = number(v, "stroke");
    a.font_size = number(v, "fontSize");
    a.text = v["text"].as_str().unwrap().into();
    a.number = v["number"].as_u64().unwrap() as u32;
    a.points = v["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| [number(p, "x"), number(p, "y")])
        .collect();
    a
}
fn close(a: f32, b: f32, context: &str) {
    assert!((a - b).abs() < 0.001, "{context}: {a} != {b}");
}
fn close_point(a: Pos2, b: &Value, context: &str) {
    close(a.x, number(b, "x"), context);
    close(a.y, number(b, "y"), context);
}
#[test]
fn original_annotation_geometry_parity() {
    let oracle: Value = serde_json::from_str(include_str!("annotations-oracle.json")).unwrap();
    for (i, f) in oracle["fixtures"].as_array().unwrap().iter().enumerate() {
        let op = f["op"].as_str().unwrap();
        let context = format!("fixture {i}: {op}");
        let expected = &f["expected"];
        if op == "windowResize" {
            let g = &f["gesture"];
            let start = &g["start"];
            let start = eframe::egui::Rect::from_min_size(
                point(start),
                vec2(number(start, "width"), number(start, "height")),
            );
            let natural = if f["type"] == "image" {
                Some(vec2(
                    number(&f["natural"], "width"),
                    number(&f["natural"], "height"),
                ))
            } else {
                None
            };
            let actual = geometry::resize_window(
                start,
                g["corner"].as_str().unwrap(),
                pos2(number(g, "startX"), number(g, "startY")),
                point(&f["point"]),
                natural,
                f["editing"].as_bool().unwrap(),
                f["shift"].as_bool().unwrap(),
            );
            for (key, value) in [
                ("x", actual.left()),
                ("y", actual.top()),
                ("width", actual.width()),
                ("height", actual.height()),
            ] {
                close(value, number(expected, key), &context);
            }
            continue;
        }
        if op == "numberColor" {
            let c = number_color(color(&f["color"]));
            let expected = color(expected);
            assert_eq!(c, expected, "{context}");
            continue;
        }
        if op == "direction" {
            close_point(
                pos2(0., 0.) + geometry::direction(point(&f["delta"]) - pos2(0., 0.)),
                expected,
                &context,
            );
            continue;
        }
        let a = annotation(&f["a"]);
        let size = vec2(
            f["width"].as_f64().unwrap_or(100.) as f32,
            f["height"].as_f64().unwrap_or(100.) as f32,
        );
        match op {
            "hit" => assert_eq!(
                a.hit(point(&f["point"]), size),
                expected.as_bool().unwrap(),
                "{context}"
            ),
            "arrow" => {
                let actual = a.arrow(size);
                let expected = expected.as_array().unwrap();
                assert_eq!(actual.len(), expected.len(), "{context}");
                for (p, e) in actual.into_iter().zip(expected) {
                    close_point(p, e, &context);
                }
            }
            "square" => close_point(
                geometry::square(a.start(), point(&f["point"]), size),
                expected,
                &context,
            ),
            "snap" => close_point(
                geometry::snap(a.start(), point(&f["point"]), size),
                expected,
                &context,
            ),
            "handles" => {
                let actual = geometry::handles(&a);
                let expected = expected.as_array().unwrap();
                assert_eq!(actual.len(), expected.len());
                for ((name, p), e) in actual.into_iter().zip(expected) {
                    assert_eq!(name, e[0].as_str().unwrap());
                    close(p.x, e[1].as_f64().unwrap() as f32, &context);
                    close(p.y, e[2].as_f64().unwrap() as f32, &context);
                }
            }
            "translate" | "resize" => {
                let actual = if op == "translate" {
                    geometry::translate(&a, point(&f["delta"]) - pos2(0., 0.))
                } else {
                    geometry::resize(
                        &a,
                        f["handle"].as_str().unwrap(),
                        point(&f["point"]),
                        size,
                        f["shift"].as_bool().unwrap(),
                        point(&f["start"]),
                    )
                };
                for (key, value) in [
                    ("x", actual.x),
                    ("y", actual.y),
                    ("x2", actual.x2),
                    ("y2", actual.y2),
                    ("radius", actual.radius),
                ] {
                    close(value, number(expected, key), &context);
                }
                for (p, e) in actual
                    .points
                    .iter()
                    .zip(expected["points"].as_array().unwrap())
                {
                    close(p[0], number(e, "x"), &context);
                    close(p[1], number(e, "y"), &context);
                }
            }
            _ => panic!("unknown operation {op}"),
        }
    }
}
#[test]
fn psd_bytes_match_original_encoder() {
    let oracle: Value = serde_json::from_str(include_str!("annotations-oracle.json")).unwrap();
    let image = image::RgbaImage::from_pixel(2, 1, image::Rgba([1, 2, 3, 255]));
    let actual = encode_psd(
        &[Layer {
            name: "原图".into(),
            left: 0,
            top: 0,
            image: image.clone(),
            hidden: true,
        }],
        &image,
    )
    .unwrap();
    let expected = oracle["psd"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

#[test]
fn canvas_raster_difference_is_bounded() {
    let oracle: Value = serde_json::from_str(include_str!("annotation-pixels.json")).unwrap();
    let bytes = include_bytes!("annotation-pixels.bin");
    let width = oracle["width"].as_u64().unwrap() as u32;
    let height = oracle["height"].as_u64().unwrap() as u32;
    let base = image::RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([(x * 2) as u8, (y * 2) as u8, 120, 255])
    });
    let stride = (width * height * 4) as usize;
    for (i, value) in oracle["items"].as_array().unwrap().iter().enumerate() {
        let a = annotation(value);
        let out = quicktool_rust::annotations::render(&base, std::slice::from_ref(&a), 0.).unwrap();
        let expected = &bytes[i * stride..(i + 1) * stride];
        let sum: u64 = out
            .as_raw()
            .iter()
            .zip(expected)
            .map(|(a, b)| a.abs_diff(*b) as u64)
            .sum();
        let mae = sum as f64 / stride as f64;
        let different = out
            .as_raw()
            .chunks_exact(4)
            .zip(expected.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        println!(
            "Canvas / Rust {:?}: MAE={mae:.4}/255, different pixels={different}/{}",
            a.tool,
            width * height
        );
        assert!(
            mae < 12.,
            "{:?} raster difference exceeded tolerance: {mae}",
            a.tool
        );
        let mut png = std::io::Cursor::new(Vec::new());
        out.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let decoded = image::load_from_memory(png.get_ref()).unwrap().to_rgba8();
        assert_eq!(decoded, out);
    }
}
