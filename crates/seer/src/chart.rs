//! The bitrate-over-time chart.

use crate::theme::{self, Theme, text};
use gpui_kit::prelude::*;
use gpui_kit::*;
use seer_core::bitrate::Analysis;
use seer_core::format;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

const HEIGHT: Pixels = px(132.);
const GRID_LINES: usize = 4;

/// Where the plot was last painted, so a pointer position can be mapped to a time.
pub type PlotBounds = Rc<Cell<Bounds<Pixels>>>;

/// `pointer` is the horizontal pointer position in window coordinates.
pub fn bitrate_chart(
    analysis: Arc<Analysis>,
    pointer: Option<Pixels>,
    plot: PlotBounds,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let t = Theme::of(cx).clone();
    let ceiling = nice_ceiling(analysis.stats.max as f32);
    let hairline = px(1.0 / window.scale_factor());

    // Position along the plot, 0 to 1, if the pointer is over it.
    let bounds = plot.get();
    let at = pointer
        .filter(|x| bounds.size.width > px(0.) && *x >= bounds.left() && *x <= bounds.right())
        .map(|x| (x - bounds.left()) / bounds.size.width);
    let readout = at.map(|at| {
        let index = ((at * analysis.overall.len() as f32) as usize).min(analysis.overall.len() - 1);
        format!(
            "{}  ·  {}",
            format::timestamp(index as f64 * analysis.interval, analysis.duration),
            format::bitrate(analysis.overall[index] as u64)
        )
    });

    let caption = || {
        div()
            .text_size(text::CAPTION)
            .line_height(text::CAPTION_LINE)
            .text_color(t.secondary_label)
            .font_features(theme::tabular())
    };

    let paint = {
        let (analysis, t, plot) = (analysis.clone(), t.clone(), plot.clone());
        canvas(
            move |bounds, _, _| plot.set(bounds),
            move |bounds, _, window, _| {
                for line in 1..=GRID_LINES {
                    let y =
                        bounds.bottom() - bounds.size.height * (line as f32 / GRID_LINES as f32);
                    window.paint_quad(fill(
                        Bounds::new(point(bounds.left(), y), size(bounds.size.width, hairline)),
                        t.separator,
                    ));
                }
                window.paint_quad(fill(
                    Bounds::new(
                        point(bounds.left(), bounds.bottom()),
                        size(bounds.size.width, hairline),
                    ),
                    t.tertiary_label,
                ));

                let points = sample(&analysis.overall, f32::from(bounds.size.width) as usize);
                if points.len() < 2 {
                    return;
                }
                let place = |i: usize, rate: f32| {
                    let x =
                        bounds.left() + bounds.size.width * (i as f32 / (points.len() - 1) as f32);
                    let y = bounds.bottom() - bounds.size.height * (rate / ceiling).min(1.);
                    point(x, y)
                };

                let mut area = PathBuilder::fill();
                area.move_to(point(bounds.left(), bounds.bottom()));
                for (i, rate) in points.iter().enumerate() {
                    area.line_to(place(i, *rate));
                }
                area.line_to(point(bounds.right(), bounds.bottom()));
                area.close();
                if let Ok(path) = area.build() {
                    window.paint_path(path, t.accent.opacity(0.22));
                }

                let mut line = PathBuilder::stroke(px(1.25));
                line.move_to(place(0, points[0]));
                for (i, rate) in points.iter().enumerate().skip(1) {
                    line.line_to(place(i, *rate));
                }
                if let Ok(path) = line.build() {
                    window.paint_path(path, t.accent);
                }

                if let Some(at) = at {
                    let x = bounds.left() + bounds.size.width * at;
                    window.paint_quad(fill(
                        Bounds::new(point(x, bounds.top()), size(hairline, bounds.size.height)),
                        t.secondary_label,
                    ));
                }
            },
        )
        .size_full()
    };

    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .flex()
                .justify_between()
                .child(caption().child(format::bitrate(ceiling as u64)))
                .child(
                    caption()
                        .text_color(t.label)
                        .child(readout.unwrap_or_default()),
                ),
        )
        .child(div().h(HEIGHT).w_full().child(paint))
        .child(
            div()
                .flex()
                .justify_between()
                .child(caption().child(format::timestamp(0., analysis.duration)))
                .child(
                    caption().child(format::timestamp(analysis.duration / 2., analysis.duration)),
                )
                .child(caption().child(format::timestamp(analysis.duration, analysis.duration))),
        )
}

/// Reduces a series to at most `columns` points, keeping each column's peak
/// so short spikes stay visible.
fn sample(rates: &[f32], columns: usize) -> Vec<f32> {
    if columns == 0 || rates.len() <= columns {
        return rates.to_vec();
    }
    (0..columns)
        .map(|c| {
            let (start, end) = (c * rates.len() / columns, (c + 1) * rates.len() / columns);
            rates[start..end.max(start + 1)]
                .iter()
                .copied()
                .fold(0., f32::max)
        })
        .collect()
}

/// Rounds up to 1, 2, 2.5 or 5 times a power of ten.
fn nice_ceiling(value: f32) -> f32 {
    if value <= 0. {
        return 1.;
    }
    let magnitude = 10f32.powf(value.log10().floor());
    let step = [1., 2., 2.5, 5., 10.]
        .into_iter()
        .find(|s| s * magnitude >= value)
        .unwrap_or(10.);
    step * magnitude
}

#[cfg(test)]
mod tests {
    // Not a glob: the kit exports its own `test` attribute.
    use super::{nice_ceiling, sample};

    #[test]
    fn ceilings() {
        assert_eq!(nice_ceiling(0.), 1.);
        assert_eq!(nice_ceiling(8_300_000.), 10_000_000.);
        assert_eq!(nice_ceiling(1_900_000.), 2_000_000.);
        assert_eq!(nice_ceiling(230_000.), 250_000.);
    }

    #[test]
    fn sampling_keeps_peaks() {
        let mut rates = vec![1.; 1000];
        rates[503] = 9.;
        let sampled = sample(&rates, 100);
        assert_eq!(sampled.len(), 100);
        assert_eq!(sampled[50], 9.);
        assert_eq!(sample(&rates, 5000).len(), 1000);
    }
}
