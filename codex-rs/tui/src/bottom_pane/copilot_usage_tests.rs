use super::*;

#[test]
fn persistent_usage_badge_states_and_widths() {
    let mut output = String::new();
    for (partial, pending) in [(false, false), (false, true), (true, false), (true, true)] {
        for width in [80, 32] {
            let usage = CopilotUsageDisplay::Available(ThreadCopilotUsageReadResponse {
                nano_aiu: 29_500_000,
                responses: 1,
                partial,
                pending,
            });
            let mut buffer = Buffer::empty(Rect::new(
                /*x*/ 0, /*y*/ 0, width, /*height*/ 1,
            ));
            badge(&usage, width).render(buffer.area, &mut buffer);
            let row: String = buffer
                .content
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect();
            output.push_str(&format!("{width} {partial} {pending}: |{row}|\n"));
        }
    }
    output.push_str(&badge(&CopilotUsageDisplay::Unavailable, /*width*/ 80).to_string());
    output.push('\n');
    output.push_str(
        &badge(
            &CopilotUsageDisplay::Available(ThreadCopilotUsageReadResponse {
                nano_aiu: 0,
                responses: 1,
                partial: true,
                pending: false,
            }),
            /*width*/ 80,
        )
        .to_string(),
    );
    insta::assert_snapshot!(output);
}

#[test]
fn usage_row_preserves_composer_and_cursor_while_idle_and_working() {
    use crate::app_event_sender::AppEventSender;
    use crate::render::renderable::Renderable;
    use pretty_assertions::assert_eq;
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let mut pane = crate::bottom_pane::tests::test_pane(AppEventSender::new(tx));
    let mut output = String::new();
    for running in [false, true] {
        pane.set_task_running(running);
        pane.set_copilot_usage(None);
        let height = pane.desired_height(/*width*/ 80);
        let cursor = pane.cursor_pos(Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, height,
        ));
        pane.set_copilot_usage(Some(CopilotUsageDisplay::Available(
            ThreadCopilotUsageReadResponse {
                nano_aiu: 29_500_000,
                responses: 1,
                partial: false,
                pending: running,
            },
        )));
        assert_eq!(pane.desired_height(/*width*/ 80), height + 1);
        let area = Rect::new(/*x*/ 0, /*y*/ 0, /*width*/ 80, height + 1);
        assert_eq!(pane.cursor_pos(area), cursor);
        let mut buffer = Buffer::empty(area);
        // ChatWidget uses this entry point rather than BottomPane's Renderable implementation.
        pane.as_renderable_with_composer_right_reserve(/*composer_right_reserve*/ 4)
            .render(area, &mut buffer);
        output.push_str(&format!("working={running}\n"));
        for row in buffer.content.chunks(/*chunk_size*/ 80) {
            let line: String = row.iter().map(ratatui::buffer::Cell::symbol).collect();
            output.push_str(line.trim_end());
            output.push('\n');
        }
    }
    insta::assert_snapshot!(output);
}
