use eframe::egui::{self, Align2, FontId, Sense, Shape, Stroke, Vec2};

/// Resolution-independent lockup; mark geometry matches assets/kozmos.svg.
pub fn sidebar_logo(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(178.0, 48.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let point = |x: f32, y: f32| rect.min + Vec2::new(x, y);
    // A restrained, single-color monogram with a deliberate vertical cut.
    let ink = crate::TEXT;
    painter.rect_filled(
        egui::Rect::from_min_max(point(9.0, 9.0), point(16.0, 39.0)),
        0.0,
        ink,
    );
    for vertices in [
        [(20.0, 24.0), (32.0, 9.0), (41.0, 9.0), (29.0, 24.0)],
        [(20.0, 24.0), (29.0, 24.0), (41.0, 39.0), (32.0, 39.0)],
    ] {
        painter.add(Shape::convex_polygon(
            vertices.into_iter().map(|(x, y)| point(x, y)).collect(),
            ink,
            Stroke::NONE,
        ));
    }
    painter.text(
        point(58.0, 24.0),
        Align2::LEFT_CENTER,
        "KOZMOS",
        FontId::proportional(23.0),
        crate::TEXT,
    );
}
