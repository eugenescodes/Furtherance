// Furtherance - Track your time without being tracked
// Copyright (C) 2025  Ricky Kresslein <rk@unobserved.io>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use crate::{
    app::Message,
    constants::{CHART_COLOR, CHART_HEIGHT, MAX_X_VALUES},
    localization::Localization,
    models::fur_task::FurTask,
};
use chrono::NaiveDate;
use iced::{widget::Row, Element, Length};
use plotters::prelude::*;
use plotters_backend::DrawingBackend;
use plotters_iced::{plotters_backend, Chart, ChartWidget};
use std::collections::BTreeMap;

use super::all_charts;

#[derive(Clone, Debug)]
pub struct SelectionEarningsRecordedChart {
    date_earned: BTreeMap<NaiveDate, f32>,
}

impl SelectionEarningsRecordedChart {
    pub fn new(tasks: &[&FurTask]) -> Self {
        Self {
            date_earned: earnings_per_day(tasks),
        }
    }

    pub fn view(&self) -> Element<Message> {
        let total_earnings: f32 = self.date_earned.values().sum();

        if self.date_earned.len() <= 1 || total_earnings == 0.0 {
            Row::new().into()
        } else {
            let chart = ChartWidget::new(self)
                .width(Length::Fill)
                .height(Length::Fixed(CHART_HEIGHT));

            chart.into()
        }
    }
}

impl Chart<Message> for SelectionEarningsRecordedChart {
    type State = ();
    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, mut chart: ChartBuilder<DB>) {
        let min_earned = self
            .date_earned
            .values()
            .copied()
            .max_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);
        let min_minus_five_percent = min_earned - (min_earned * 0.05);
        let max_earned = self
            .date_earned
            .values()
            .copied()
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);

        if self.date_earned.len() > 1 {
            if let Some(first_date) = self.date_earned.first_key_value() {
                if let Some(last_date) = self.date_earned.last_key_value() {
                    let localization = Localization::new();

                    let mut chart = chart
                        .margin(30)
                        .caption(
                            localization.get_message("earnings-for-selection-title", None),
                            ("sans-serif", 15)
                                .into_font()
                                .color(&all_charts::light_dark_color()),
                        )
                        .x_label_area_size(30)
                        .y_label_area_size(30)
                        .build_cartesian_2d(
                            *first_date.0..*last_date.0,
                            min_minus_five_percent..max_earned,
                        )
                        .unwrap();

                    chart
                        .configure_mesh()
                        .label_style(&all_charts::light_dark_color())
                        .x_label_style(
                            ("sans-serif", 12)
                                .into_font()
                                .color(&all_charts::light_dark_color()),
                        )
                        .x_labels(MAX_X_VALUES)
                        .y_label_style(
                            ("sans-serif", 12)
                                .into_font()
                                .color(&all_charts::light_dark_color())
                                .transform(FontTransform::Rotate90),
                        )
                        .y_label_formatter(&|y| format!("${:.2}", y))
                        .axis_style(
                            ShapeStyle::from(all_charts::light_dark_color()).stroke_width(1),
                        )
                        .draw()
                        .unwrap();

                    chart
                        .draw_series(LineSeries::new(
                            self.date_earned.iter().map(|(d, t)| (*d, *t)),
                            CHART_COLOR.filled(),
                        ))
                        .unwrap();
                }
            }
        }
    }
}

fn earnings_per_day(tasks: &[&FurTask]) -> BTreeMap<NaiveDate, f32> {
    let mut earnings_by_day = BTreeMap::new();
    for task in tasks {
        *earnings_by_day
            .entry(task.start_time.date_naive())
            .or_insert(0.0) += task.total_earnings();
    }
    earnings_by_day
}
