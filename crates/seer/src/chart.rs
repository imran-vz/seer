//! The bitrate-over-time chart: Seer's signature view. A headline with the
//! measured average and peak; a key line naming the trace ("All streams")
//! with the readout's sample at its trailing edge; the plot with a dashed
//! average line and a peak marker; a value gutter labelled at 0, half and
//! the ceiling (unit once); and time ticks underneath. Trace, marker,
//! readout and ticks share one normalised timeline.
//!
//! The trace's colour says what it carries. A file whose data is all video
//! (or all audio) draws its trace and area in that data hue. A file with
//! both draws the total as a neutral line over a stacked area: video's part
//! in the video hue, audio's in the audio hue, so a mixed aggregate never
//! passes for a video-only one.
//!
//! The readout follows the pointer, or the keyboard when the chart has
//! focus (`Readout::Sample`, moved by the inspector). [`spoken_value`] puts
//! the same sample (or the headline) into words for assistive technology.

use crate::theme::{self, Theme, size, text};
use crate::ui;
use gpui_kit::prelude::*;
use gpui_kit::*;
use seer_core::bitrate::Analysis;
use seer_core::format;
use seer_core::probe::StreamKind;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

/// Where the plot was last painted, so a pointer position can be mapped to a time.
pub type PlotBounds = Rc<Cell<Bounds<Pixels>>>;

/// What the readout shows: the plotted point under the pointer, or a
/// plotted point chosen from the keyboard (an index into the points as
/// plotted at the plot's last width; see [`samples`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Readout {
    /// The horizontal pointer position in window coordinates.
    Pointer(Pixels),
    Sample(usize),
}

/// How many points the plot shows at its last painted width: the steps the
/// keyboard readout moves through.
pub fn samples(analysis: &Analysis, plot: &PlotBounds) -> usize {
    Timeline::of(analysis)
        .picks(&analysis.overall, columns(plot.get()))
        .len()
}

/// The plotted point nearest a horizontal window position, as an index for
/// [`Readout::Sample`]. `None` when the plot has not been painted.
pub fn sample_at(analysis: &Analysis, plot: &PlotBounds, x: Pixels) -> Option<usize> {
    let bounds = plot.get();
    if bounds.size.width <= px(0.) {
        return None;
    }
    let at = ((x - bounds.left()) / bounds.size.width).clamp(0., 1.);
    let points = Timeline::of(analysis).points(&analysis.overall, columns(bounds));
    nearest_index(&points, at)
}

/// The plotted point a readout names, as (position, bits per second):
/// the one under the pointer while it is over the plot, or the one the
/// keyboard chose. `None` with no readout or before the plot is painted.
fn sample_of(
    analysis: &Analysis,
    readout: Option<Readout>,
    bounds: Bounds<Pixels>,
) -> Option<(f32, f32)> {
    let plotted = || Timeline::of(analysis).points(&analysis.overall, columns(bounds));
    match readout {
        Some(Readout::Pointer(x))
            if bounds.size.width > px(0.) && x >= bounds.left() && x <= bounds.right() =>
        {
            let at = (x - bounds.left()) / bounds.size.width;
            nearest(&plotted(), at)
        }
        Some(Readout::Sample(index)) => {
            let points = plotted();
            points
                .get(index.min(points.len().saturating_sub(1)))
                .copied()
        }
        _ => None,
    }
}

/// What the chart says to assistive technology: the readout's sample
/// ("0:42, 8.2 Mbps") while there is one, otherwise the headline ("Average
/// 5.1 Mbps, peak 9.3 Mbps at 1:02"). The same words the chart shows.
pub fn spoken_value(analysis: &Analysis, readout: Option<Readout>, plot: &PlotBounds) -> String {
    let timeline = Timeline::of(analysis);
    match sample_of(analysis, readout, plot.get()) {
        Some((at, rate)) => format!(
            "{}, {}",
            format::timestamp(timeline.time(at), timeline.duration),
            format::bitrate(rate as u64)
        ),
        None => format!(
            "Average {}, peak {} at {}",
            format::bitrate(analysis.stats.average),
            format::bitrate(analysis.stats.max),
            format::timestamp(analysis.stats.peak_at, timeline.duration)
        ),
    }
}

/// Everything on the plot sits on one timeline (`Timeline`): the trace's
/// points, the peak marker, the readout's sample and the time ticks.
pub fn bitrate_chart(
    analysis: Arc<Analysis>,
    readout: Option<Readout>,
    plot: PlotBounds,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let t = Theme::of(cx).clone();
    let timeline = Timeline::of(&analysis);
    let ceiling = nice_ceiling(analysis.stats.max as f32);
    let (divisor, unit) = unit(ceiling);
    let ticks: [(f32, String); 3] = [
        (1., axis_number(ceiling / divisor)),
        (0.5, axis_number(ceiling / 2. / divisor)),
        (0., "0".to_owned()),
    ];
    // The gutter is as wide as its widest label.
    let gutter = ticks
        .iter()
        .map(|(_, label)| label.as_str())
        .chain([unit])
        .map(|label| caption_width(label, window))
        .fold(px(0.), Pixels::max);
    // The trace's ink, and the area under it: one hue for a single kind of
    // data; for video and audio together a neutral line over the stacked
    // parts (painted below).
    let series = series_kind(&analysis);
    let (hue, hue_fill) = match series {
        Series::Only(StreamKind::Video) => (t.data_video, t.data_video_fill),
        Series::Only(StreamKind::Audio) => (t.data_audio, t.data_audio_fill),
        Series::Mixed => (t.label, t.fill),
        _ => (t.secondary_label, t.fill),
    };

    // The plotted point the readout names: under the pointer while it is
    // over the plot, or the one the keyboard chose. The readout line, the
    // rule, its dot and the spoken value all use this one point.
    let sample = sample_of(&analysis, readout, plot.get());

    let caption = || {
        div()
            .text_size(text::CAPTION)
            .line_height(text::CAPTION_LINE)
            .text_color(t.secondary_label)
            .font_features(theme::tabular())
            .whitespace_nowrap()
    };
    let small = || {
        div()
            .text_size(text::SMALL)
            .line_height(text::SMALL_LINE)
            .font_features(theme::tabular())
            .whitespace_nowrap()
    };

    // Average first, in the strongest type on the tab; the peak on the line
    // below. Both stay put while the pointer moves: its sample has a line of
    // its own above the plot. Each carries the key of its mark in the plot.
    let dash_key = div()
        .flex_none()
        .w(size::ICON_SMALL)
        .border_t(size::HAIRLINE)
        .border_dashed()
        .border_color(t.secondary_label);
    let dot_key = |color: Hsla| {
        div()
            .flex_none()
            .size(size::BAR_THICK)
            .rounded_full()
            .bg(color)
    };
    let headline = div()
        .flex()
        .flex_col()
        .gap(size::GAP_TINY)
        .child(
            div()
                .flex()
                .items_center()
                .gap(size::GAP_SMALL)
                .child(
                    div()
                        .text_size(text::BODY)
                        .line_height(text::BODY_LINE)
                        .font_weight(FontWeight::SEMIBOLD)
                        .font_features(theme::tabular())
                        .text_color(t.label)
                        .child(format::bitrate(analysis.stats.average)),
                )
                .child(dash_key)
                .child(small().text_color(t.secondary_label).child("average")),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(size::GAP_SMALL)
                .child(dot_key(hue))
                .child(small().text_color(t.secondary_label).child("Peak"))
                .child(
                    small()
                        .text_color(t.label)
                        .child(format::bitrate(analysis.stats.max)),
                )
                .child(small().text_color(t.secondary_label).child(format!(
                    "at {}",
                    format::timestamp(analysis.stats.peak_at, timeline.duration)
                ))),
        );

    let paint = {
        let (analysis, t, plot) = (analysis.clone(), t.clone(), plot.clone());
        canvas(
            move |bounds, _, _| plot.set(bounds),
            move |bounds, _, window, _| {
                let line_at = |fraction: f32| bounds.bottom() - bounds.size.height * fraction;
                let x_at = |fraction: f32| bounds.left() + bounds.size.width * fraction;
                let y_of = |rate: f32| line_at((rate / ceiling).clamp(0., 1.));
                let rule = |y: Pixels, color: Hsla, window: &mut Window| {
                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.left(), y),
                            size(bounds.size.width, size::HAIRLINE),
                        ),
                        color,
                    ));
                };
                // Gridlines at half and the ceiling, the baseline at 0: the
                // three values the gutter names.
                rule(line_at(1.), t.separator, window);
                rule(line_at(0.5), t.separator, window);
                rule(bounds.bottom() - size::HAIRLINE, t.border, window);

                let picks = timeline.picks(&analysis.overall, columns(bounds));
                // The trace spans the whole plot: an interval's rate holds
                // from its start to the next, and the last one to the end.
                let span = |rates: &[f32]| -> Vec<(f32, f32)> {
                    let mut points: Vec<(f32, f32)> = picks
                        .iter()
                        .map(|&i| (timeline.at(i), rates.get(i).copied().unwrap_or(0.)))
                        .collect();
                    if let (Some(&(first_at, first)), Some(&(last_at, last))) =
                        (points.first(), points.last())
                    {
                        if first_at > 0. {
                            points.insert(0, (0., first));
                        }
                        if last_at < 1. {
                            points.push((1., last));
                        }
                    }
                    points
                };
                let points = span(&analysis.overall);
                if points.is_empty() {
                    return;
                }
                let place = |(at, rate): (f32, f32)| point(x_at(at), y_of(rate));

                // The area between two curves on the same timeline (the
                // lower one `None` for the baseline).
                let band = |upper: &[(f32, f32)],
                            lower: Option<&[(f32, f32)]>,
                            color: Hsla,
                            window: &mut Window| {
                    let mut area = PathBuilder::fill();
                    match lower {
                        None => {
                            area.move_to(point(bounds.left(), bounds.bottom()));
                            for p in upper {
                                area.line_to(place(*p));
                            }
                            area.line_to(point(bounds.right(), bounds.bottom()));
                        }
                        Some(lower) => {
                            area.move_to(place(upper[0]));
                            for p in upper.iter().skip(1) {
                                area.line_to(place(*p));
                            }
                            for p in lower.iter().rev() {
                                area.line_to(place(*p));
                            }
                        }
                    }
                    area.close();
                    if let Ok(path) = area.build() {
                        window.paint_path(path, color);
                    }
                };
                match series {
                    // Video, then audio stacked on it, then anything else
                    // (subtitles, data) up to the total.
                    Series::Mixed => {
                        let (video, audio) = kind_rates(&analysis);
                        let both: Vec<f32> = video.iter().zip(&audio).map(|(v, a)| v + a).collect();
                        let (video, both) = (span(&video), span(&both));
                        band(&video, None, t.data_video_fill, window);
                        band(&both, Some(&video), t.data_audio_fill, window);
                        band(&points, Some(&both), t.fill, window);
                    }
                    _ => band(&points, None, hue_fill, window),
                }

                // The average, under the series so the trace stays on top.
                let average = y_of(analysis.stats.average as f32);
                let mut dashed = PathBuilder::stroke(size::HAIRLINE)
                    .dash_array(&[size::CHART_DASH, size::CHART_DASH]);
                dashed.move_to(point(bounds.left(), average));
                dashed.line_to(point(bounds.right(), average));
                if let Ok(path) = dashed.build() {
                    window.paint_path(path, t.secondary_label);
                }

                let mut line = PathBuilder::stroke(size::CHART_STROKE);
                line.move_to(place(points[0]));
                for p in points.iter().skip(1) {
                    line.line_to(place(*p));
                }
                if let Ok(path) = line.build() {
                    window.paint_path(path, hue);
                }

                // A dot on the trace, ringed in the pane's colour so it
                // reads over the line.
                let dot = |centre: Point<Pixels>, color: Hsla, window: &mut Window| {
                    let radius = size::BAR_THICK * 0.5 + size::HAIRLINE;
                    window.paint_quad(
                        fill(
                            Bounds::new(
                                point(centre.x - radius, centre.y - radius),
                                size(radius * 2., radius * 2.),
                            ),
                            color,
                        )
                        .corner_radii(radius)
                        .border_widths(size::HAIRLINE)
                        .border_color(t.surface_raised),
                    );
                };

                if let Some((index, rate)) = peak(&analysis.overall) {
                    dot(place((timeline.at(index), rate)), hue, window);
                }

                if let Some((at, rate)) = sample {
                    window.paint_quad(fill(
                        Bounds::new(
                            point(x_at(at), bounds.top()),
                            size(size::HAIRLINE, bounds.size.height),
                        ),
                        t.secondary_label,
                    ));
                    dot(place((at, rate)), t.label, window);
                }
            },
        )
        .size_full()
    };

    // Each value label is centred on its gridline.
    let scale = div()
        .relative()
        .flex_none()
        .w(gutter)
        .h(size::CHART_HEIGHT)
        .children(ticks.into_iter().map(|(fraction, label)| {
            caption()
                .absolute()
                .right_0()
                .top(size::CHART_HEIGHT * (1. - fraction) - text::CAPTION_LINE * 0.5)
                .child(label)
        }));

    // Over the plot: the unit above the gutter, then what the trace is, and
    // while the pointer is over the plot, the sample under it at the
    // trailing edge. The line is always there, so nothing moves.
    let series_key = div()
        .flex()
        .items_center()
        .gap(size::GAP_SMALL)
        .child(
            div()
                .flex_none()
                .w(size::ICON_SMALL)
                .h(size::CHART_STROKE)
                .bg(hue),
        )
        .child(small().text_color(t.secondary_label).child("All streams"));
    let readout = sample.map(|(at, rate)| {
        div()
            .flex()
            .items_center()
            .gap(size::GAP_SMALL)
            .child(
                small()
                    .text_color(t.secondary_label)
                    .child(format::timestamp(timeline.time(at), timeline.duration)),
            )
            .child(
                small()
                    .text_color(t.label)
                    .child(format::bitrate(rate as u64)),
            )
    });
    let key_line = div()
        .flex()
        .items_end()
        .gap(size::GAP_SMALL)
        .h(text::SMALL_LINE)
        .child(
            div()
                .flex()
                .flex_none()
                .justify_end()
                .w(gutter)
                .child(caption().child(unit)),
        )
        .child(
            div()
                .flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .justify_between()
                .gap(size::GAP)
                .child(series_key)
                .children(readout),
        );

    let plot_area = div()
        .flex()
        .flex_col()
        // A full gap keeps the unit clear of the ceiling label, which is
        // centred on the top gridline.
        .gap(size::GAP)
        .child(key_line)
        .child(
            div()
                .flex()
                .gap(size::GAP_SMALL)
                .child(scale)
                .child(div().flex_1().min_w_0().h(size::CHART_HEIGHT).child(paint)),
        );
    // Start, middle and end of the timeline, each on its own position: three
    // equal thirds, so the middle label is centred on the plot's middle.
    let tick = |at: f32| caption().child(format::timestamp(timeline.time(at), timeline.duration));
    let time_axis = div()
        .flex()
        .gap(size::GAP_SMALL)
        .child(div().flex_none().w(gutter))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .child(div().flex_1().flex().justify_start().child(tick(0.)))
                .child(div().flex_1().flex().justify_center().child(tick(0.5)))
                .child(div().flex_1().flex().justify_end().child(tick(1.))),
        );

    div()
        .flex()
        .flex_col()
        .gap(size::GAP)
        .child(headline)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(size::GAP_SMALL)
                .child(plot_area)
                .child(time_axis),
        )
}

/// What the aggregate trace carries, which decides its colour.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Series {
    /// Only this kind of media data (video or audio).
    Only(StreamKind),
    /// Video and audio together: a neutral line over the stacked parts.
    Mixed,
    /// Neither (subtitles or data only): neutral.
    Other,
}

fn series_kind(analysis: &Analysis) -> Series {
    let carries = |kind| {
        analysis
            .streams
            .iter()
            .any(|s| s.kind == kind && s.total_bytes > 0)
    };
    match (carries(StreamKind::Video), carries(StreamKind::Audio)) {
        (true, true) => Series::Mixed,
        (true, false) => Series::Only(StreamKind::Video),
        (false, true) => Series::Only(StreamKind::Audio),
        (false, false) => Series::Other,
    }
}

/// Video's and audio's rates summed per interval, on the overall series'
/// intervals.
fn kind_rates(analysis: &Analysis) -> (Vec<f32>, Vec<f32>) {
    let len = analysis.overall.len();
    let sum = |kind: StreamKind| {
        let mut total = vec![0f32; len];
        for series in analysis.streams.iter().filter(|s| s.kind == kind) {
            for (sum, rate) in total.iter_mut().zip(&series.rates) {
                if rate.is_finite() {
                    *sum += rate;
                }
            }
        }
        total
    };
    (sum(StreamKind::Video), sum(StreamKind::Audio))
}

/// One column per point of plot width.
fn columns(bounds: Bounds<Pixels>) -> usize {
    f32::from(bounds.size.width).max(0.) as usize
}

/// The plot's one coordinate system: a position from 0 (the start of the
/// file) to 1 (its end). Interval `i` sits at its start time, `i ×
/// interval`, which is also where the analysis says the peak is.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Timeline {
    duration: f64,
    interval: f64,
}

impl Timeline {
    fn of(analysis: &Analysis) -> Timeline {
        let interval = analysis.interval.max(f64::EPSILON);
        let duration = match analysis.duration {
            d if d > 0. => d,
            _ => analysis.overall.len() as f64 * interval,
        };
        Timeline {
            duration: duration.max(f64::EPSILON),
            interval,
        }
    }

    /// Where interval `index` starts on the plot.
    fn at(&self, index: usize) -> f32 {
        ((index as f64 * self.interval) / self.duration).clamp(0., 1.) as f32
    }

    /// The time, in seconds, at a position on the plot.
    fn time(&self, at: f32) -> f64 {
        at.clamp(0., 1.) as f64 * self.duration
    }

    /// The series as plotted: (position, bits per second), in order. With
    /// more intervals than `columns`, each column keeps its highest interval
    /// at that interval's own position, so short spikes stay visible and the
    /// peak marker sits on the trace.
    fn points(&self, rates: &[f32], columns: usize) -> Vec<(f32, f32)> {
        self.picks(rates, columns)
            .into_iter()
            .map(|i| (self.at(i), rates[i]))
            .collect()
    }

    /// The intervals [`Timeline::points`] plots, in order, so other series
    /// on the same intervals (video's and audio's parts) plot at the same
    /// positions.
    fn picks(&self, rates: &[f32], columns: usize) -> Vec<usize> {
        if columns == 0 || rates.len() <= columns {
            return (0..rates.len()).collect();
        }
        let mut best: Vec<Option<(usize, f32)>> = vec![None; columns];
        for (i, rate) in rates.iter().copied().enumerate() {
            if !rate.is_finite() {
                continue;
            }
            let column = ((self.at(i) * columns as f32) as usize).min(columns - 1);
            match best[column] {
                Some((_, kept)) if kept > rate => {}
                _ => best[column] = Some((i, rate)),
            }
        }
        best.into_iter().flatten().map(|(i, _)| i).collect()
    }
}

/// The plotted point closest to a position on the plot.
fn nearest(points: &[(f32, f32)], at: f32) -> Option<(f32, f32)> {
    nearest_index(points, at).map(|i| points[i])
}

/// Where in `points` the one closest to a position on the plot is.
fn nearest_index(points: &[(f32, f32)], at: f32) -> Option<usize> {
    points
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| (a.0 - at).abs().total_cmp(&(b.0 - at).abs()))
        .map(|(i, _)| i)
}

/// Width of a caption-sized label in the UI font.
fn caption_width(label: &str, window: &Window) -> Pixels {
    ui::text_width(
        &label.to_owned().into(),
        text::CAPTION,
        FontWeight::NORMAL,
        window,
    )
    .ceil()
}

/// The highest value and where it is.
fn peak(rates: &[f32]) -> Option<(usize, f32)> {
    rates
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, rate)| rate.is_finite())
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

/// The axis unit for a ceiling in bits per second, and what to divide by.
fn unit(ceiling: f32) -> (f32, &'static str) {
    if ceiling >= 1_000_000. {
        (1_000_000., "Mbps")
    } else if ceiling >= 1_000. {
        (1_000., "kbps")
    } else {
        (1., "bps")
    }
}

/// An axis value without trailing zeros: "20", "2.5", "1.25".
fn axis_number(value: f32) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
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
    use super::{
        PlotBounds, Readout, Series, Timeline, axis_number, kind_rates, nearest, nearest_index,
        nice_ceiling, peak, series_kind, spoken_value, unit,
    };
    use seer_core::bitrate::{Analysis, Series as Track, Stats};
    use seer_core::format;
    use seer_core::probe::StreamKind;

    fn analysis(kinds: &[(StreamKind, u64, Vec<f32>)]) -> Analysis {
        let streams: Vec<Track> = kinds
            .iter()
            .enumerate()
            .map(|(i, (kind, bytes, rates))| Track {
                stream_index: i,
                kind: *kind,
                codec: String::new(),
                language: None,
                total_bytes: *bytes,
                rates: rates.clone(),
            })
            .collect();
        let len = streams.iter().map(|s| s.rates.len()).max().unwrap_or(0);
        let overall = (0..len)
            .map(|i| streams.iter().filter_map(|s| s.rates.get(i)).sum())
            .collect();
        Analysis {
            duration: len as f64,
            interval: 1.,
            overall,
            stats: Stats {
                average: 0,
                median: 0,
                min: 0,
                max: 0,
                peak_at: 0.,
            },
            streams,
            packets: 0,
        }
    }

    #[test]
    fn a_mixed_trace_is_not_a_video_trace() {
        let video = (StreamKind::Video, 10, vec![8., 9.]);
        let audio = (StreamKind::Audio, 1, vec![1., 1.]);
        assert_eq!(
            series_kind(&analysis(&[video.clone(), audio.clone()])),
            Series::Mixed
        );
        assert_eq!(
            series_kind(&analysis(std::slice::from_ref(&video))),
            Series::Only(StreamKind::Video)
        );
        assert_eq!(
            series_kind(&analysis(std::slice::from_ref(&audio))),
            Series::Only(StreamKind::Audio)
        );
        // The stacked parts are each kind's own rates.
        let mixed = analysis(&[video, audio]);
        assert_eq!(kind_rates(&mixed), (vec![8., 9.], vec![1., 1.]));
    }

    #[test]
    fn the_keyboard_and_the_pointer_read_the_same_points() {
        let points = [(0., 1.), (0.5, 4.), (1., 2.)];
        assert_eq!(nearest_index(&points, 0.6), Some(1));
        assert_eq!(nearest(&points, 0.6), Some(points[1]));
        assert_eq!(nearest_index(&[], 0.5), None);
    }

    #[test]
    fn ceilings() {
        assert_eq!(nice_ceiling(0.), 1.);
        assert_eq!(nice_ceiling(8_300_000.), 10_000_000.);
        assert_eq!(nice_ceiling(1_900_000.), 2_000_000.);
        assert_eq!(nice_ceiling(230_000.), 250_000.);
    }

    #[test]
    fn one_timeline_for_trace_peak_and_ticks() {
        // 10 s in 0.25 s intervals: interval 20 starts at 5 s, the middle.
        let timeline = Timeline {
            duration: 10.,
            interval: 0.25,
        };
        assert_eq!(timeline.at(0), 0.);
        assert_eq!(timeline.at(20), 0.5);
        assert_eq!(timeline.time(0.5), 5.);
        assert_eq!(timeline.time(1.), 10.);
        // Past the declared end it stays on the plot.
        assert_eq!(timeline.at(400), 1.);
    }

    #[test]
    fn downsampling_keeps_peaks_where_they_are() {
        let timeline = Timeline {
            duration: 100.,
            interval: 0.1,
        };
        let mut rates = vec![1.; 1000];
        rates[503] = 9.;
        let points = timeline.points(&rates, 100);
        assert_eq!(points.len(), 100);
        // Other series plot on the same intervals.
        assert_eq!(timeline.picks(&rates, 100).len(), points.len());
        // The spike keeps its own position, the one the peak marker uses.
        let (index, rate) = peak(&rates).unwrap();
        assert!(points.contains(&(timeline.at(index), rate)));
        assert!(points.windows(2).all(|w| w[0].0 < w[1].0));
        assert_eq!(timeline.points(&rates, 5000).len(), 1000);
    }

    #[test]
    fn the_pointer_reads_the_plotted_point() {
        let points = [(0., 1.), (0.5, 4.), (1., 2.)];
        assert_eq!(nearest(&points, 0.6), Some((0.5, 4.)));
        assert_eq!(nearest(&points, 0.9), Some((1., 2.)));
        assert_eq!(nearest(&[], 0.5), None);
    }

    #[test]
    fn axis_labels_name_the_unit_once() {
        assert_eq!(unit(20_000_000.), (1_000_000., "Mbps"));
        assert_eq!(unit(250_000.), (1_000., "kbps"));
        assert_eq!(unit(1.), (1., "bps"));
        assert_eq!(axis_number(20.), "20");
        assert_eq!(axis_number(2.5), "2.5");
        assert_eq!(axis_number(1.25), "1.25");
        assert_eq!(axis_number(125.), "125");
        assert_eq!(axis_number(0.5), "0.5");
    }

    #[test]
    fn the_spoken_value_is_the_readout_or_the_headline() {
        let mut mixed = analysis(&[(StreamKind::Video, 10, vec![1_000_000., 3_000_000.])]);
        mixed.stats.average = 2_000_000;
        mixed.stats.max = 3_000_000;
        mixed.stats.peak_at = 1.;
        // Not painted yet: every interval is a plotted point.
        let plot = PlotBounds::default();
        let at = |seconds: f64| format::timestamp(seconds, 2.);
        assert_eq!(
            spoken_value(&mixed, Some(Readout::Sample(1)), &plot),
            format!("{}, {}", at(1.), format::bitrate(3_000_000))
        );
        // Past the end, the last point; with no readout, the headline.
        assert_eq!(
            spoken_value(&mixed, Some(Readout::Sample(9)), &plot),
            spoken_value(&mixed, Some(Readout::Sample(1)), &plot)
        );
        assert_eq!(
            spoken_value(&mixed, None, &plot),
            format!(
                "Average {}, peak {} at {}",
                format::bitrate(2_000_000),
                format::bitrate(3_000_000),
                at(1.)
            )
        );
    }

    #[test]
    fn peak_is_the_highest_point() {
        assert_eq!(peak(&[1., 4., 2.]), Some((1, 4.)));
        assert_eq!(peak(&[]), None);
    }
}
