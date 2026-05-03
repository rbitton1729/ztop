//! Pools tab — unified tree view. Manual per-row rendering (one
//! `Line` of styled `Span`s per visible row) because pool rows and
//! vdev rows have different column shapes — the ratatui `Table`
//! widget assumes a single column schema across all rows. The NAME
//! column starts at the same x-offset on both row types (load-bearing
//! for the tree indent); other columns float per row type. `▼`/`▶`
//! glyphs mark expandable pool rows; vdev rows have a blank glyph
//! slot to anchor the NAME column.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use super::widgets;
use crate::app::{format_bytes, App, PoolsView, VisibleRow};
use crate::pools::{PoolHealth, PoolInfo, ScrubState, VdevKind, VdevNode, VdevState};

const WIDE_THRESHOLD: u16 = 100;

pub(super) fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().borders(Borders::ALL).title("Pools");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(err) = &app.pools_init_error {
        widgets::draw_centered(
            frame,
            inner,
            &format!("libzfs unavailable: {err}"),
            Style::default().fg(Color::Red),
        );
        return;
    }
    if app.pools_snapshot.is_empty() {
        widgets::draw_centered(
            frame,
            inner,
            "(no pools imported)",
            Style::default().fg(Color::DarkGray),
        );
        return;
    }

    let wide = inner.width >= WIDE_THRESHOLD;
    let rows = app.flatten_visible_pool_rows();
    let selected_idx = match &app.pools_view {
        PoolsView::Tree { selected, .. } => *selected,
        PoolsView::Detail { .. } => 0,
    };
    let expanded = match &app.pools_view {
        PoolsView::Tree { expanded, .. } => expanded,
        PoolsView::Detail { expanded, .. } => expanded,
    };

    // Layout: header(1) + body(rest).
    let [header_area, body_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(inner);

    let header_line = build_header_line(wide);
    frame.render_widget(Paragraph::new(header_line), header_area);

    let body_lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let is_selected = i == selected_idx;
            let line = match row {
                VisibleRow::Pool(p) => build_pool_line(p, expanded, wide),
                VisibleRow::Vdev { node, depth } => build_vdev_line(node, *depth, wide),
            };
            if is_selected {
                line.style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                line
            }
        })
        .collect();

    frame.render_widget(Paragraph::new(body_lines), body_area);
}

fn build_header_line(wide: bool) -> Line<'static> {
    let bold = Style::default().add_modifier(Modifier::BOLD);
    // Pool-row header. Vdev rows borrow these column slots semantically:
    //   NAME → indented vdev name
    //   HEALTH → STATE (same x-offset)
    //   TYPE → KIND
    //   CAPACITY → SIZE + DEVICE_PATH (vdev rows reclaim this width)
    //   FRAG → R errors      ┐
    //   SCRUB → W errors     ├ vdev rows split the right edge
    //   ERR → C errors        ┘ into three R/W/C cells (wide mode)
    let mut spans = vec![
        Span::raw("  "), // marker(2)
        Span::styled(format!("{:<2}", ""), bold), // glyph slot(1)+space(1)
        Span::styled(format!("{:<14}", "NAME"), bold),
        Span::styled(format!("{:<10}", "HEALTH"), bold),
        Span::styled(format!("{:<8}", "TYPE"), bold),
        Span::styled(format!("{:<28}", "CAPACITY"), bold),
    ];
    if wide {
        spans.push(Span::styled(format!("{:<6}", "FRAG"), bold));
        spans.push(Span::styled(format!("{:<16}", "SCRUB"), bold));
        spans.push(Span::styled(format!("{:<6}", "ERR"), bold));
    } else {
        spans.push(Span::styled(format!("{:<16}", "SCRUB"), bold));
        // Trailing 6-char blank slot — corresponds to the ERR cell on
        // narrow vdev rows. Header is intentionally blank here because pool
        // narrow rows don't have an ERR column; only vdev narrow rows do.
        spans.push(Span::styled(format!("{:<6}", ""), bold));
    }
    Line::from(spans)
}

fn build_pool_line<'a>(
    p: &'a PoolInfo,
    expanded: &std::collections::BTreeSet<String>,
    wide: bool,
) -> Line<'a> {
    let glyph = if p.root_vdev.children.is_empty() {
        ' '
    } else if expanded.contains(&p.name) {
        '▼'
    } else {
        '▶'
    };
    let health_label = pool_health_label(p.health);
    let raid_type = p.raid_label();
    let capacity = render_capacity_cell(p);
    let scrub = render_scrub_cell(&p.scrub);

    let mut spans = vec![
        Span::raw("  "), // marker — selection-bg picks it up; selected-row marker overlay handled by selection style
        Span::raw(format!("{glyph} ")),
        Span::raw(format!("{:<14}", truncate_pad(&p.name, 14))),
        Span::styled(
            format!("{:<10}", health_label),
            widgets::pool_health_style(p.health),
        ),
        Span::raw(format!("{:<8}", truncate_pad(&raid_type, 8))),
        Span::raw(format!("{:<28}", capacity)),
    ];
    if wide {
        let frag = match p.fragmentation_pct {
            Some(v) => format!("{v}%"),
            None => "—".into(),
        };
        spans.push(Span::raw(format!("{:<6}", frag)));
        spans.push(Span::raw(format!("{:<16}", truncate_pad(&scrub, 16))));
        let err_sum = p.root_vdev.total_errors();
        let err_style = if err_sum == 0 {
            Style::default()
        } else {
            Style::default().fg(Color::Red)
        };
        spans.push(Span::styled(format!("{:<6}", err_sum), err_style));
    } else {
        spans.push(Span::raw(format!("{:<16}", truncate_pad(&scrub, 16))));
        // Trailing 6-char blank to match the vdev narrow row's ERR(6) cell
        // — keeps body widths equal so selection bg extends evenly across
        // pool and vdev rows.
        spans.push(Span::raw(" ".repeat(6)));
    }
    Line::from(spans)
}

/// Truncate `s` to `width` chars and right-pad to that width with spaces.
/// Truncates from the right (loses the tail).
fn truncate_pad(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        format!("{s:<width$}")
    } else {
        let truncated: String = s.chars().take(width).collect();
        truncated
    }
}

fn render_capacity_cell(p: &PoolInfo) -> String {
    let used = format_bytes(p.allocated_bytes);
    let total = format_bytes(p.size_bytes);
    let frac = p.capacity_fraction().clamp(0.0, 1.0);
    let filled = (frac * 6.0).round() as usize;
    let filled = filled.min(6);
    let mini: String = "█".repeat(filled) + &"░".repeat(6 - filled);
    format!("{used}/{total} [{mini}]")
}

fn render_scrub_cell(scrub: &ScrubState) -> String {
    match scrub {
        ScrubState::Never => "—".into(),
        ScrubState::Error => "error".into(),
        ScrubState::InProgress {
            progress_pct,
            is_resilver,
            ..
        } => {
            if *is_resilver {
                format!("resilver {progress_pct}%")
            } else {
                format!("scrub {progress_pct}%")
            }
        }
        ScrubState::Finished { completed_at, .. } => format_relative_time(*completed_at),
    }
}

fn format_relative_time(when: std::time::SystemTime) -> String {
    match when.elapsed() {
        Ok(dur) => {
            let days = dur.as_secs() / 86_400;
            if days == 0 {
                "today".into()
            } else if days < 30 {
                format!("{days}d ago")
            } else if days < 365 {
                format!("{} mo ago", days / 30)
            } else {
                format!("{}y ago", days / 365)
            }
        }
        Err(_) => "—".into(),
    }
}

fn pool_health_label(health: PoolHealth) -> String {
    match health {
        PoolHealth::Online => "ONLINE",
        PoolHealth::Degraded => "DEGRADED",
        PoolHealth::Faulted => "FAULTED",
        PoolHealth::Offline => "OFFLINE",
        PoolHealth::Removed => "REMOVED",
        PoolHealth::Unavail => "UNAVAIL",
    }
    .to_string()
}

fn build_vdev_line<'a>(
    node: &'a VdevNode,
    depth: u8,
    wide: bool,
) -> Line<'a> {
    let indent = "  ".repeat(depth as usize);
    // Pool row layout: marker(2) + glyph+space(2) + NAME(14) + HEALTH(10) + ...
    // Vdev row layout: marker(2) + indent + glyph+space(2) + NAME(rest of pool's NAME slot minus indent) + STATE(10) + KIND(8) + ...
    // The NAME slot for vdev rows starts at the same x as pool rows (col 4)
    // and consumes `14 - indent_chars` chars before the STATE column.
    let name_width = 14usize.saturating_sub(indent.chars().count());
    let state_label = if is_group(node.kind) {
        String::new()
    } else {
        vdev_state_label(node.state).to_string()
    };
    let kind_label = vdev_kind_label(node.kind);
    let size = match node.size_bytes {
        Some(b) => format_bytes(b),
        None => String::new(),
    };

    let mut spans = vec![
        Span::raw("  "),
        Span::raw(indent.clone()),
        Span::raw("  "), // glyph+space slot — always blank on vdev rows
        Span::raw(format!("{:<width$}", truncate_pad(&node.name, name_width), width = name_width)),
        Span::styled(
            format!("{:<10}", state_label),
            vdev_state_style(node.state),
        ),
        Span::raw(format!("{:<8}", truncate_pad(&kind_label, 8))),
        Span::raw(format!("{:<8}", truncate_pad(&size, 8))),
    ];

    if wide {
        // DEVICE_PATH absorbs the slack from CAPACITY+FRAG+SCRUB on pool rows,
        // minus the SIZE we already emitted: 28 + 6 + 16 - 8 = 42. Then R/W/C
        // (6+6+6 = 18) lands at the same right edge as the pool row's
        // FRAG+SCRUB+ERR (6+16+6 = 28), minus the 10 cols we use for the
        // wider DEVICE_PATH. Total wide row body width matches pool row's
        // 88-char body, so the selection bg extends evenly.
        //
        // Math: NAME(14) + STATE(10) + KIND(8) + SIZE(8) + DEVICE_PATH(30) + R(6) + W(6) + C(6) = 88
        //       Pool:    NAME(14) + HEALTH(10) + TYPE(8) + CAPACITY(28)  + FRAG(6) + SCRUB(16) + ERR(6) = 88
        let path_width: usize = 30;
        let device_path = match (node.kind, node.device_path.as_ref()) {
            (VdevKind::Disk | VdevKind::File | VdevKind::LogVdev | VdevKind::CacheVdev | VdevKind::SpareVdev,
             Some(path)) => truncate_left_with_ellipsis(path, path_width),
            _ => " ".repeat(path_width), // interior nodes have no device path
        };
        spans.push(Span::raw(format!("{:<width$}", device_path, width = path_width)));

        let (read, write, checksum) = if is_group(node.kind) {
            (String::new(), String::new(), String::new())
        } else {
            (
                format!("{}", node.errors.read),
                format!("{}", node.errors.write),
                format!("{}", node.errors.checksum),
            )
        };
        spans.push(Span::styled(
            format!("{:<6}", read),
            err_style(node.errors.read),
        ));
        spans.push(Span::styled(
            format!("{:<6}", write),
            err_style(node.errors.write),
        ));
        spans.push(Span::styled(
            format!("{:<6}", checksum),
            err_style(node.errors.checksum),
        ));
    } else {
        // Narrow: drop DEVICE_PATH; combine R/W/C into a single ERR cell.
        // The CAPACITY-width slot (28 chars) shrinks: keep the SIZE we
        // already emitted in the 8-char SIZE cell, leave the rest blank
        // (no DEVICE_PATH on narrow).
        let blank: String = " ".repeat(28 - 8);
        spans.push(Span::raw(blank));
        // The SCRUB(16) slot stays blank on vdev narrow.
        spans.push(Span::raw(format!("{:<16}", "")));
        let err_sum = if is_group(node.kind) {
            String::new()
        } else {
            format!("{}", node.errors.sum())
        };
        spans.push(Span::styled(
            format!("{:<6}", err_sum),
            err_style(node.errors.sum()),
        ));
    }
    Line::from(spans)
}

/// Truncate from the *left* with a leading `…` so trailing identifying
/// chars (the GUID tail of a `wwn-0x500abc...`) stay visible. If the
/// string already fits, return it as-is (right-padded by the caller).
fn truncate_left_with_ellipsis(s: &str, max_width: usize) -> String {
    let len = s.chars().count();
    if len <= max_width {
        return s.to_string();
    }
    if max_width <= 1 {
        return "…".chars().take(max_width).collect();
    }
    let take = max_width - 1; // reserve 1 char for `…`
    let skip = len - take;
    let mut out = String::with_capacity(max_width * 4);
    out.push('…');
    out.extend(s.chars().skip(skip));
    out
}

fn err_style(n: u64) -> Style {
    if n == 0 {
        Style::default()
    } else {
        Style::default().fg(Color::Red)
    }
}

fn is_group(kind: VdevKind) -> bool {
    matches!(
        kind,
        VdevKind::LogGroup | VdevKind::CacheGroup | VdevKind::SpareGroup
    )
}

fn vdev_state_label(state: VdevState) -> &'static str {
    match state {
        VdevState::Online => "ONLINE",
        VdevState::Degraded => "DEGRADED",
        VdevState::Faulted => "FAULTED",
        VdevState::Offline => "OFFLINE",
        VdevState::Removed => "REMOVED",
        VdevState::Unavail => "UNAVAIL",
    }
}

fn vdev_state_style(state: VdevState) -> Style {
    match state {
        VdevState::Online => Style::default().fg(Color::Green),
        VdevState::Degraded => Style::default().fg(Color::Yellow),
        VdevState::Faulted | VdevState::Removed | VdevState::Unavail => {
            Style::default().fg(Color::Red)
        }
        VdevState::Offline => Style::default().fg(Color::DarkGray),
    }
}

fn vdev_kind_label(kind: VdevKind) -> String {
    match kind {
        VdevKind::Root => "root".into(),
        VdevKind::Raidz { parity } => format!("raidz{parity}"),
        VdevKind::Mirror => "mirror".into(),
        VdevKind::Disk => "disk".into(),
        VdevKind::File => "file".into(),
        VdevKind::LogGroup => "logs".into(),
        VdevKind::LogVdev => "log".into(),
        VdevKind::CacheGroup => "cache".into(),
        VdevKind::CacheVdev => "cache".into(),
        VdevKind::SpareGroup => "spares".into(),
        VdevKind::SpareVdev => "spare".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, PoolsView, Tab};
    use crate::arcstats;
    use crate::meminfo::{self, MemSource};
    use crate::pools::fake::FakePoolsSource;
    use crate::pools::{
        ErrorCounts, PoolHealth, PoolInfo, PoolsSource, ScrubState, VdevKind,
        VdevNode, VdevState,
    };
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::PathBuf;

    fn leaf(name: &str, errors: ErrorCounts, device_path: Option<&str>) -> VdevNode {
        VdevNode {
            name: name.into(),
            kind: VdevKind::Disk,
            state: VdevState::Online,
            size_bytes: Some(256 * 1024 * 1024 * 1024),
            errors,
            children: vec![],
            device_path: device_path.map(|s| s.to_string()),
        }
    }

    fn raidz1_pool(name: &str, leaves: Vec<VdevNode>) -> PoolInfo {
        PoolInfo {
            name: name.into(),
            health: PoolHealth::Online,
            allocated_bytes: 500 * 1024 * 1024 * 1024,
            size_bytes: 1024 * 1024 * 1024 * 1024,
            free_bytes: 524 * 1024 * 1024 * 1024,
            fragmentation_pct: Some(8),
            scrub: ScrubState::Never,
            errors: ErrorCounts::default(),
            root_vdev: VdevNode {
                name: name.into(),
                kind: VdevKind::Root,
                state: VdevState::Online,
                size_bytes: Some(1024 * 1024 * 1024 * 1024),
                errors: ErrorCounts::default(),
                children: vec![VdevNode {
                    name: "raidz1-0".into(),
                    kind: VdevKind::Raidz { parity: 1 },
                    state: VdevState::Online,
                    size_bytes: Some(1024 * 1024 * 1024 * 1024),
                    errors: ErrorCounts::default(),
                    children: leaves,
                    device_path: None,
                }],
                device_path: None,
            },
        }
    }

    fn app_for_tree(
        pools: Vec<PoolInfo>,
        pools_init_error: Option<String>,
        expanded_names: &[&str],
    ) -> App {
        let arcstats_path = PathBuf::from("fixtures/arcstats");
        let meminfo_path = PathBuf::from("fixtures/meminfo");
        let arc_reader: Box<dyn FnMut() -> anyhow::Result<arcstats::ArcStats>> = {
            let p = arcstats_path.clone();
            Box::new(move || arcstats::linux::from_procfs_path(&p))
        };
        let mem: Option<Box<dyn MemSource>> = Some(Box::new(
            meminfo::linux::LinuxMemSource::new(meminfo_path),
        ));
        let pools_source: Option<Box<dyn PoolsSource>> =
            Some(Box::new(FakePoolsSource::new(pools.clone())));
        let mut app = App::new(arc_reader, mem, pools_source, pools_init_error, None, None)
            .expect("fixture App::new");
        app.current_tab = Tab::Pools;
        // Override default-expand: clear, then insert exactly the names asked for.
        if let PoolsView::Tree { expanded, selected } = &mut app.pools_view {
            expanded.clear();
            for n in expanded_names {
                expanded.insert((*n).to_string());
            }
            *selected = 0;
        }
        app.pools_snapshot = pools;
        app
    }

    fn whole_text(backend: &TestBackend) -> String {
        let buf = backend.buffer();
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render(app: &App, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| draw(frame, frame.area(), app))
            .expect("draw");
        whole_text(terminal.backend())
    }

    #[test]
    fn empty_snapshot_shows_no_pools_notice() {
        let app = app_for_tree(vec![], None, &[]);
        let out = render(&app, 100, 24);
        assert!(out.contains("no pools imported"));
    }

    #[test]
    fn libzfs_unavailable_shows_init_error() {
        let app = app_for_tree(vec![], Some("init failed".to_string()), &[]);
        let out = render(&app, 100, 24);
        assert!(out.contains("libzfs unavailable"));
    }

    #[test]
    fn collapsed_pool_shows_only_pool_row() {
        let pool = raidz1_pool("tank", vec![leaf("sda", ErrorCounts::default(), None)]);
        let app = app_for_tree(vec![pool], None, &[]); // not expanded
        let out = render(&app, 120, 24);
        assert!(out.contains("tank"));
        assert!(!out.contains("raidz1-0"), "should not show raidz when collapsed");
        assert!(!out.contains("sda"), "should not show leaf when collapsed");
        assert!(out.contains("▶"), "collapsed pool should show ▶ glyph");
    }

    #[test]
    fn expanded_pool_shows_vdev_rows() {
        let pool = raidz1_pool("tank", vec![leaf("sda", ErrorCounts::default(), None)]);
        let app = app_for_tree(vec![pool], None, &["tank"]);
        let out = render(&app, 120, 24);
        assert!(out.contains("tank"));
        assert!(out.contains("raidz1-0"));
        assert!(out.contains("sda"));
        assert!(out.contains("▼"), "expanded pool should show ▼ glyph");
    }

    #[test]
    fn vdev_row_shows_device_path() {
        let pool = raidz1_pool(
            "tank",
            vec![leaf(
                "sda",
                ErrorCounts::default(),
                Some("/dev/disk/by-id/wwn-0x500abc"),
            )],
        );
        let app = app_for_tree(vec![pool], None, &["tank"]);
        let out = render(&app, 140, 24);
        assert!(
            out.contains("/dev/disk/by-id/wwn-0x500abc")
                || out.contains("by-id/wwn-0x500abc"),
            "device path missing or truncated unexpectedly: {out:?}"
        );
    }

    #[test]
    fn device_path_truncates_with_ellipsis_when_overflowing() {
        let long_path =
            "/dev/disk/by-id/wwn-0x500abc123def4567890aaabbbccc1234567890dddeeefff";
        let pool = raidz1_pool(
            "tank",
            vec![leaf("sda", ErrorCounts::default(), Some(long_path))],
        );
        let app = app_for_tree(vec![pool], None, &["tank"]);
        // 102 cols outer → 100 cols inner = exactly hits the wide-mode
        // threshold, but the device_path slot is only 20 chars wide so a
        // 60-char path has to truncate.
        let out = render(&app, 102, 24);
        // Ellipsis should appear, and the trailing portion should be visible.
        assert!(
            out.contains("…"),
            "expected leading ellipsis on truncated path: {out:?}"
        );
        assert!(
            out.contains("dddeeefff") || out.contains("eeefff"),
            "expected trailing characters of long path: {out:?}"
        );
    }

    #[test]
    fn rwc_columns_color_red_when_nonzero() {
        let pool = raidz1_pool(
            "tank",
            vec![leaf(
                "sda",
                ErrorCounts {
                    read: 5,
                    write: 0,
                    checksum: 0,
                },
                None,
            )],
        );
        let app = app_for_tree(vec![pool], None, &["tank"]);
        let backend = TestBackend::new(140, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| draw(frame, frame.area(), &app))
            .expect("draw");
        // Find the cell containing "5" on the sda row and check its fg color.
        let buf = terminal.backend().buffer();
        let mut found_red_5 = false;
        for y in 0..buf.area.height {
            let row: String = (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect();
            if row.contains("sda") {
                for x in 0..buf.area.width {
                    let cell = &buf[(x, y)];
                    if cell.symbol() == "5" && cell.fg == Color::Red {
                        found_red_5 = true;
                        break;
                    }
                }
                break;
            }
        }
        assert!(found_red_5, "expected red '5' in R column on sda row");
    }

    #[test]
    fn group_header_rows_blank_size_and_errors() {
        // Build a pool with a log group containing one log vdev.
        let log_vdev = VdevNode {
            name: "nvme0n1p1".into(),
            kind: VdevKind::LogVdev,
            state: VdevState::Online,
            size_bytes: Some(8 * 1024 * 1024 * 1024),
            errors: ErrorCounts::default(),
            children: vec![],
            device_path: Some("/dev/nvme0n1p1".into()),
        };
        let log_group = VdevNode {
            name: "logs".into(),
            kind: VdevKind::LogGroup,
            state: VdevState::Online,
            size_bytes: None,
            errors: ErrorCounts::default(),
            children: vec![log_vdev],
            device_path: None,
        };
        let mut pool = raidz1_pool("tank", vec![]);
        pool.root_vdev.children.push(log_group);
        let app = app_for_tree(vec![pool], None, &["tank"]);
        let out = render(&app, 140, 24);
        // The 'logs' group row should show its kind label but no size cell content.
        let logs_line = out
            .lines()
            .find(|l| l.contains("logs") && !l.contains("nvme"))
            .expect("missing logs group row");
        // The size column starts after STATE+KIND. Just assert the row has no
        // numeric byte unit in the SIZE cell.
        assert!(
            !logs_line.contains(" GiB ") && !logs_line.contains(" TiB "),
            "logs group row should have blank SIZE: {logs_line:?}"
        );
    }

    #[test]
    fn selection_highlight_persists_across_pool_and_vdev_rows() {
        let pool = raidz1_pool("tank", vec![leaf("sda", ErrorCounts::default(), None)]);
        let mut app = app_for_tree(vec![pool], None, &["tank"]);
        // Move selection to sda (row 2: pool, raidz, sda).
        if let PoolsView::Tree { selected, .. } = &mut app.pools_view {
            *selected = 2;
        }
        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| draw(frame, frame.area(), &app))
            .expect("draw");
        let buf = terminal.backend().buffer();
        // Find the row containing "sda" and assert at least one cell in that
        // row has DarkGray bg.
        let mut found_dark_bg = false;
        for y in 0..buf.area.height {
            let row: String = (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect();
            if row.contains("sda") {
                for x in 0..buf.area.width {
                    if buf[(x, y)].bg == Color::DarkGray {
                        found_dark_bg = true;
                        break;
                    }
                }
                break;
            }
        }
        assert!(
            found_dark_bg,
            "selected sda row should have DarkGray bg cells"
        );
    }

    #[test]
    fn narrow_layout_drops_columns() {
        let pool = raidz1_pool(
            "tank",
            vec![leaf("sda", ErrorCounts::default(), Some("/dev/sda"))],
        );
        let app = app_for_tree(vec![pool], None, &["tank"]);
        let out = render(&app, 80, 24);
        assert!(!out.contains("FRAG"), "narrow should drop FRAG header");
        assert!(!out.contains(" ERR "), "narrow should drop ERR header column");
        assert!(
            !out.contains("/dev/sda"),
            "narrow should drop DEVICE_PATH on vdev rows: {out:?}"
        );
    }

    #[test]
    fn default_expanded_visible_after_first_paint() {
        // Use the real default-expand path (App::new + first refresh insert).
        let arcstats_path = PathBuf::from("fixtures/arcstats");
        let meminfo_path = PathBuf::from("fixtures/meminfo");
        let arc_reader: Box<dyn FnMut() -> anyhow::Result<arcstats::ArcStats>> = {
            let p = arcstats_path.clone();
            Box::new(move || arcstats::linux::from_procfs_path(&p))
        };
        let mem: Option<Box<dyn MemSource>> = Some(Box::new(
            meminfo::linux::LinuxMemSource::new(meminfo_path),
        ));
        let pool = raidz1_pool("tank", vec![leaf("sda", ErrorCounts::default(), None)]);
        let pools_source: Option<Box<dyn PoolsSource>> =
            Some(Box::new(FakePoolsSource::new(vec![pool.clone()])));
        let mut app = App::new(arc_reader, mem, pools_source, None, None, None)
            .expect("App::new");
        app.current_tab = Tab::Pools;
        app.pools_snapshot = vec![pool];
        let out = render(&app, 120, 24);
        assert!(out.contains("▼"), "default-expand should show ▼ on first paint: {out:?}");
        assert!(out.contains("raidz1-0"), "vdev rows should be visible by default: {out:?}");
    }
}
