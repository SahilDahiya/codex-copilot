//! A dedicated bottom row keeps usage visible during work, hints, and popup views.
use super::BottomPane;
use crate::render::renderable::FlexRenderable;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableItem;
use codex_app_server_protocol::ThreadCopilotUsageReadResponse;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Widget;

pub(crate) enum CopilotUsageDisplay {
    Loading,
    Available(ThreadCopilotUsageReadResponse),
    Unavailable,
}

impl BottomPane {
    pub(crate) fn set_copilot_usage(&mut self, usage: Option<CopilotUsageDisplay>) {
        self.copilot_usage = usage;
    }

    pub(crate) fn as_renderable_with_composer_right_reserve(
        &self,
        composer_right_reserve: u16,
    ) -> RenderableItem<'_> {
        let body = self.body_with_composer_right_reserve(composer_right_reserve);
        let Some(usage) = &self.copilot_usage else {
            return body;
        };
        let mut flex = FlexRenderable::new();
        flex.push(/*flex*/ 1, body);
        flex.push(
            /*flex*/ 0,
            RenderableItem::Owned(Box::new(CopilotBadge(usage))),
        );
        RenderableItem::Owned(Box::new(flex))
    }
}

struct CopilotBadge<'a>(&'a CopilotUsageDisplay);

impl Renderable for CopilotBadge<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        badge(self.0, area.width).render(area, buf);
    }
    fn desired_height(&self, _width: u16) -> u16 {
        1
    }
}

fn badge(usage: &CopilotUsageDisplay, width: u16) -> Line<'static> {
    let text = match usage {
        CopilotUsageDisplay::Available(usage)
            if usage.partial && usage.nano_aiu == 0 && usage.estimated_nano_usd.is_none() =>
        {
            if usage.pending {
                "Copilot cost unavailable · pending".to_string()
            } else {
                "Copilot cost unavailable".to_string()
            }
        }
        CopilotUsageDisplay::Available(usage) => {
            // One Copilot AI credit is $0.01; estimates are already nano USD.
            let nano_usd = i128::from(usage.nano_aiu.max(0)) / 100
                + i128::from(usage.estimated_nano_usd.unwrap_or(0).max(0));
            let dollars = (nano_usd + 50_000) / 100_000;
            let prefix = if usage.estimated_nano_usd.is_some() {
                "Est. "
            } else {
                ""
            };
            let state = match (usage.partial, usage.pending) {
                (true, true) => " partial · pending",
                (true, false) => " partial",
                (false, true) => " pending",
                (false, false) => "",
            };
            let usd = format!("{}.{:04}", dollars / 10_000, dollars % 10_000);
            let full = format!("Copilot {prefix}${usd}{state}");
            if full.chars().count() <= usize::from(width) {
                full
            } else {
                let compact = format!("{prefix}${usd}{state}");
                if compact.chars().count() <= usize::from(width) {
                    compact
                } else {
                    format!("{prefix}${usd}")
                }
            }
        }
        CopilotUsageDisplay::Loading => "Copilot usage loading…".to_string(),
        CopilotUsageDisplay::Unavailable => "Copilot usage unavailable".to_string(),
    };
    Line::from(text.dim()).right_aligned()
}

#[cfg(test)]
#[path = "copilot_usage_tests.rs"]
mod tests;
