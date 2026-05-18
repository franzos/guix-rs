use iced::widget::{
    button, column, container, progress_bar, row, scrollable, text, tooltip, Column, Space,
};
use iced::{Alignment, Element, Font, Length};

use crate::app::{
    bootstrap_help_message, op_supports_cancel, ActiveOp, App, Message, CANCEL_PKEXEC_TOOLTIP,
};
use crate::progress_summary::{BuildStatus, ProgressSummary, Stage};

pub fn view<'a>(app: &'a App, op: &'a ActiveOp) -> Element<'a, Message> {
    let summary = &op.progress;
    let title_line = title_row(op, summary);
    let stage_line = stage_row(summary);

    let mut body: Column<'a, Message> = Column::new().spacing(10).width(Length::Fill);
    body = body.push(title_line);
    body = body.push(stage_line);

    if let Some(running) = running_builds_section(summary) {
        body = body.push(running);
    }
    if let Some(downloads) = downloads_section(summary) {
        body = body.push(downloads);
    }
    if let Some(downloads_done) = finished_downloads_section(summary) {
        body = body.push(downloads_done);
    }
    if let Some(done) = finished_builds_section(summary) {
        body = body.push(done);
    }
    if let Some(last) = summary.last_status_line.as_deref() {
        let hint: Element<'_, Message> = text(format!("Last: {last}"))
            .size(12)
            .style(text::secondary)
            .into();
        body = body.push(hint);
    }

    if app.show_bootstrap_help() {
        body = body.push(bootstrap_help_block(app));
    }

    let scroll = scrollable(body).height(Length::Fill);
    let footer = footer_row(app, op);

    container(
        column![scroll, footer]
            .spacing(10)
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn title_row<'a>(op: &'a ActiveOp, summary: &'a ProgressSummary) -> Element<'a, Message> {
    let title = format!("{} (op #{})", op.kind.label(), op.id.0);
    let elapsed = summary
        .elapsed()
        .map(format_elapsed)
        .unwrap_or_else(|| "—".to_string());
    row![
        text(title).size(20),
        Space::new().width(Length::Fill),
        text(elapsed).size(14).style(text::secondary),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

fn stage_row<'a>(summary: &'a ProgressSummary) -> Element<'a, Message> {
    let counts = format!(
        "{built}/{started} built, {dl_done}/{dl_started} downloaded ({mb:.1} MB)",
        built = summary.build_count_done,
        started = summary.build_count_started,
        dl_done = summary.download_count_done,
        dl_started = summary.download_count_started,
        mb = (summary.bytes_downloaded as f64) / (1024.0 * 1024.0),
    );
    let header = row![
        text(summary.stage.label()).size(16),
        Space::new().width(Length::Fill),
        text(counts).size(12).style(text::secondary),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let bar: Element<'_, Message> = match summary.percent_complete() {
        Some(p) => progress_bar(0.0..=1.0, p).girth(8).into(),
        None => progress_bar(0.0..=1.0, 0.0).girth(8).into(),
    };

    column![header, bar].spacing(6).into()
}

fn running_builds_section<'a>(summary: &'a ProgressSummary) -> Option<Element<'a, Message>> {
    let running: Vec<&_> = summary
        .builds
        .values()
        .filter(|b| b.status == BuildStatus::Running)
        .collect();
    if running.is_empty() {
        return None;
    }
    let mut col: Column<'a, Message> = Column::new().spacing(2);
    col = col.push(text(format!("Running ({}):", running.len())).size(14));
    for b in running {
        let line: Element<'_, Message> = text(format!("  - {} [building]", b.pretty_name))
            .size(12)
            .font(Font::MONOSPACE)
            .into();
        col = col.push(line);
    }
    Some(col.into())
}

fn finished_builds_section<'a>(summary: &'a ProgressSummary) -> Option<Element<'a, Message>> {
    let finished: Vec<&_> = summary
        .builds
        .values()
        .filter(|b| b.status != BuildStatus::Running)
        .collect();
    if finished.is_empty() {
        return None;
    }
    let header = format!(
        "Finished ({} done, {} failed):",
        summary.build_count_done, summary.build_count_failed
    );
    let mut col: Column<'a, Message> = Column::new().spacing(2);
    col = col.push(text(header).size(14));

    let total = finished.len();
    let take_from = total.saturating_sub(3);
    if total > 5 {
        col = col.push(
            text(format!("  ... and {} more", take_from))
                .size(12)
                .style(text::secondary),
        );
    }
    for b in &finished[take_from..] {
        let marker = match b.status {
            BuildStatus::Done => "done",
            BuildStatus::Failed => "FAILED",
            BuildStatus::Running => "building",
        };
        let line: Element<'_, Message> = text(format!("  - {} [{}]", b.pretty_name, marker))
            .size(12)
            .font(Font::MONOSPACE)
            .into();
        col = col.push(line);
    }
    Some(col.into())
}

fn downloads_section<'a>(summary: &'a ProgressSummary) -> Option<Element<'a, Message>> {
    let active: Vec<&_> = summary.downloads.values().filter(|d| !d.done).collect();
    if active.is_empty() {
        return None;
    }
    let mut col: Column<'a, Message> = Column::new().spacing(4);
    col = col.push(text(format!("Active downloads ({}):", active.len())).size(14));
    for d in active {
        let label = match d.bytes_total {
            Some(total) if total > 0 => format!(
                "  {}    {:.1}/{:.1} MB",
                d.pretty_name,
                (d.bytes_done as f64) / (1024.0 * 1024.0),
                (total as f64) / (1024.0 * 1024.0),
            ),
            _ => format!(
                "  {}    {:.1} MB",
                d.pretty_name,
                (d.bytes_done as f64) / (1024.0 * 1024.0),
            ),
        };
        let line: Element<'_, Message> = text(label).size(12).font(Font::MONOSPACE).into();
        col = col.push(line);
        match d.bytes_total {
            Some(total) if total > 0 => {
                let frac = (d.bytes_done as f32 / total as f32).clamp(0.0, 1.0);
                let bar: Element<'_, Message> = progress_bar(0.0..=1.0, frac).girth(6).into();
                col = col.push(bar);
            }
            _ => {
                let bar: Element<'_, Message> = progress_bar(0.0..=1.0, 0.0).girth(6).into();
                col = col.push(bar);
            }
        }
    }
    Some(col.into())
}

fn finished_downloads_section<'a>(summary: &'a ProgressSummary) -> Option<Element<'a, Message>> {
    let finished: Vec<&_> = summary.downloads.values().filter(|d| d.done).collect();
    if finished.is_empty() {
        return None;
    }
    let mut col: Column<'a, Message> = Column::new().spacing(2);
    col = col.push(text(format!("Completed downloads ({}):", finished.len())).size(14));
    let total = finished.len();
    let take_from = total.saturating_sub(3);
    if total > 5 {
        col = col.push(
            text(format!("  ... and {} more", take_from))
                .size(12)
                .style(text::secondary),
        );
    }
    for d in &finished[take_from..] {
        let size_hint = match d.bytes_total {
            Some(total) if total > 0 => {
                format!(
                    "  {}    {:.1} MB",
                    d.pretty_name,
                    (total as f64) / (1024.0 * 1024.0)
                )
            }
            _ => format!("  {}", d.pretty_name),
        };
        let line: Element<'_, Message> = text(size_hint).size(12).font(Font::MONOSPACE).into();
        col = col.push(line);
    }
    Some(col.into())
}

fn bootstrap_help_block<'a>(app: &'a App) -> Element<'a, Message> {
    let help = bootstrap_help_message(
        app.auto_load_path().as_deref(),
        app.settings.source_config_path.as_deref(),
    );
    let mut help_col: Column<'a, Message> = Column::new().spacing(2);
    for ln in help.lines() {
        let line: Element<'_, Message> = text(ln.to_string()).size(12).font(Font::MONOSPACE).into();
        help_col = help_col.push(line);
    }
    help_col.into()
}

fn footer_row<'a>(app: &'a App, op: &'a ActiveOp) -> Element<'a, Message> {
    let mut footer = row![].spacing(8);

    if !op.finished {
        let supports_cancel = op_supports_cancel(op.kind);
        let on_press = if supports_cancel && op.cancel.is_some() {
            Some(Message::CancelClicked)
        } else {
            None
        };
        let cancel_btn = button(text("Cancel")).on_press_maybe(on_press);
        let cancel_el: Element<'a, Message> = if supports_cancel {
            cancel_btn.into()
        } else {
            tooltip(
                cancel_btn,
                container(text(CANCEL_PKEXEC_TOOLTIP))
                    .padding(6)
                    .style(container::rounded_box),
                tooltip::Position::Top,
            )
            .into()
        };
        footer = footer
            .push(cancel_el)
            .push(text(running_status_text(&op.progress)));
    } else {
        let summary_text = match op.progress.failure.as_deref() {
            Some(msg) => msg.to_string(),
            None => match op.final_code {
                Some(0) => "Done.".to_string(),
                Some(code) => format!("Failed (exit {code})."),
                None => "Ended without exit summary.".to_string(),
            },
        };
        footer = footer
            .push(button(text("Close")).on_press(Message::DismissOverlay))
            .push(text(summary_text));
    }

    let log_label = if app.show_log { "Hide log" } else { "Show log" };
    footer = footer
        .push(Space::new().width(Length::Fill))
        .push(button(text(log_label)).on_press(Message::ToggleLog));

    footer.align_y(Alignment::Center).into()
}

fn running_status_text(summary: &ProgressSummary) -> String {
    match summary.stage {
        Stage::Starting => "Starting...".into(),
        Stage::Done => "Done.".into(),
        Stage::Failed => "Failed.".into(),
        s => format!("{}...", s.label()),
    }
}

fn format_elapsed(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation_subscription::{OpId, SharedOp};
    use libguix::ProgressEvent;

    fn fake_op(stage_events: &[ProgressEvent]) -> (App, ActiveOp) {
        let (app, _) = App::new();
        let kind = crate::operation_subscription::OpKind::Pull;
        let mut progress = ProgressSummary::new(kind);
        for e in stage_events {
            progress.ingest(e);
        }
        let op = ActiveOp {
            id: OpId(42),
            kind,
            cancel: None,
            op_slot: SharedOp::new_empty_for_tests(),
            final_code: None,
            finished: false,
            bootstrap_likely: false,
            progress,
            channel_shadow_seen: false,
        };
        (app, op)
    }

    #[test]
    fn view_renders_in_starting_stage() {
        let (app, op) = fake_op(&[]);
        let _ = view(&app, &op);
    }

    #[test]
    fn view_renders_in_downloading_stage() {
        let (app, op) = fake_op(&[ProgressEvent::SubstituteDownload {
            item: "/gnu/store/abc-foo".into(),
            bytes_done: 100,
            bytes_total: Some(1000),
        }]);
        let _ = view(&app, &op);
    }

    #[test]
    fn view_renders_in_building_stage() {
        let (app, op) = fake_op(&[ProgressEvent::BuildStart {
            drv: "/gnu/store/abc-foo.drv".into(),
        }]);
        let _ = view(&app, &op);
    }

    #[test]
    fn view_renders_in_failed_stage() {
        let (app, op) = fake_op(&[ProgressEvent::BuildFailed {
            drv: "/gnu/store/abc-foo.drv".into(),
            log_path: Some("/var/log/foo".into()),
        }]);
        let _ = view(&app, &op);
    }

    #[test]
    fn view_renders_with_completed_downloads() {
        let (app, op) = fake_op(&[
            ProgressEvent::SubstituteDownload {
                item: "/gnu/store/abc-foo".into(),
                bytes_done: 0,
                bytes_total: Some(1000),
            },
            ProgressEvent::SubstituteDownloadDone {
                item: "/gnu/store/abc-foo".into(),
                bytes_total: Some(1000),
            },
        ]);
        let _ = view(&app, &op);
    }

    #[test]
    fn view_renders_in_done_stage() {
        let (app, op) = fake_op(&[ProgressEvent::ExitSummary {
            code: 0,
            duration_secs: 1.0,
        }]);
        let _ = view(&app, &op);
    }
}
