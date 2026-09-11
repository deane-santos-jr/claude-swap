use crate::actions::{self, Action, Outcome};
use crate::cswap;
use crate::desktop_sessions::{list_desktop_sessions, DesktopSession};
use crate::launchd;
use crate::model::{AccountRow, AutoEvent, ListPayload, Spend, Window};
use crate::timefmt;
use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

const USAGE_REFRESH: Duration = Duration::from_secs(60);
const SESSION_REFRESH: Duration = Duration::from_secs(10);
const BACKGROUND_REFRESH: Duration = Duration::from_secs(5);

pub struct App {
    tx: Sender<Outcome>,
    rx: Receiver<Outcome>,
    accounts: Vec<AccountRow>,
    active_number: Option<u32>,
    last_usage_refresh: Option<Instant>,
    last_session_refresh: Instant,
    desktop_sessions: Vec<DesktopSession>,
    in_flight: Option<Action>,
    notice: Option<Notice>,
    confirm_remove: Option<u32>,
    background_auto: bool,
    background_checked_at: Option<Instant>,
    auto_last_event: Option<String>,
    threshold: f64,
    threshold_draft: f64,
    resume_target: Option<u32>,
    loaded_once: bool,
}

struct Notice {
    text: String,
    is_error: bool,
    at: Instant,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (tx, rx) = channel();
        cc.egui_ctx.set_zoom_factor(1.0);
        let threshold = read_threshold();
        let mut app = Self {
            tx,
            rx,
            accounts: Vec::new(),
            active_number: None,
            last_usage_refresh: None,
            last_session_refresh: Instant::now(),
            desktop_sessions: list_desktop_sessions(),
            in_flight: None,
            notice: None,
            confirm_remove: None,
            background_auto: launchd::is_running(),
            background_checked_at: Some(Instant::now()),
            auto_last_event: launchd::last_log_line(),
            threshold,
            threshold_draft: threshold,
            resume_target: None,
            loaded_once: false,
        };
        app.dispatch(Action::Refresh);
        app
    }

    fn dispatch(&mut self, action: Action) {
        if self.in_flight.is_some() {
            return;
        }
        self.in_flight = Some(action.clone());
        actions::spawn(action, self.tx.clone());
    }

    fn drain_outcomes(&mut self) {
        while let Ok(outcome) = self.rx.try_recv() {
            match outcome {
                Outcome::Accounts(payload) => self.apply_accounts(payload),
                Outcome::Notice(text) => self.set_notice(text, false),
                Outcome::Failure(text) => self.set_notice(text, true),
                Outcome::AutoEvents(events) => self.apply_auto_events(events),
                Outcome::Done(action) => {
                    match action {
                        Action::SetThreshold(_) => self.threshold = self.threshold_draft,
                        Action::EnableBackgroundAuto | Action::DisableBackgroundAuto => {
                            self.background_checked_at = None;
                        }
                        _ => {}
                    }
                    self.in_flight = None;
                }
            }
        }
    }

    fn apply_accounts(&mut self, payload: ListPayload) {
        self.active_number = payload.active_account_number;
        self.accounts = payload.accounts;
        self.last_usage_refresh = Some(Instant::now());
        self.loaded_once = true;
        if self.resume_target.is_none() {
            self.resume_target = self.suggested_resume_target();
        }
    }

    fn apply_auto_events(&mut self, events: Vec<AutoEvent>) {
        if let Some(last) = events.iter().rev().find(|e| e.event != "poll" && e.event != "sleep") {
            let summary = last.summary();
            let is_error = last.event == "error";
            self.auto_last_event = Some(summary.clone());
            if last.event == "switch" {
                self.set_notice(format!("Auto-switch: {summary}"), false);
            } else if is_error {
                self.set_notice(format!("Auto-switch: {summary}"), true);
            }
        }
    }

    fn set_notice(&mut self, text: String, is_error: bool) {
        self.notice = Some(Notice { text, is_error, at: Instant::now() });
    }

    fn suggested_resume_target(&self) -> Option<u32> {
        self.accounts
            .iter()
            .filter(|a| !a.disabled && a.usage_status == "ok")
            .min_by(|a, b| a.worst_window_pct().partial_cmp(&b.worst_window_pct()).unwrap())
            .map(|a| a.number)
    }

    fn tick(&mut self) {
        if self.in_flight.is_none() {
            let due = self
                .last_usage_refresh
                .map(|t| t.elapsed() >= USAGE_REFRESH)
                .unwrap_or(false);
            if due {
                self.dispatch(Action::Refresh);
            }
        }
        if self.last_session_refresh.elapsed() >= SESSION_REFRESH {
            self.desktop_sessions = list_desktop_sessions();
            self.last_session_refresh = Instant::now();
        }
        let stale = self.background_checked_at.map(|t| t.elapsed() >= BACKGROUND_REFRESH).unwrap_or(true);
        if stale && self.in_flight.is_none() {
            self.background_auto = launchd::is_running();
            if self.background_auto {
                self.auto_last_event = launchd::last_log_line();
            }
            self.background_checked_at = Some(Instant::now());
        }
        if let Some(n) = &self.notice {
            if !n.is_error && n.at.elapsed() > Duration::from_secs(12) {
                self.notice = None;
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_outcomes();
        self.tick();
        let ctx = ui.ctx().clone();
        ctx.request_repaint_after(Duration::from_secs(1));
        let panel_fill = ui.visuals().panel_fill;

        egui::Panel::top("header")
            .frame(Frame::new().inner_margin(Margin::symmetric(16, 12)).fill(panel_fill))
            .show(ui, |ui| self.header(ui));

        egui::Panel::bottom("footer")
            .frame(Frame::new().inner_margin(Margin::symmetric(16, 10)).fill(panel_fill))
            .show(ui, |ui| self.footer(ui));

        egui::CentralPanel::default()
            .frame(Frame::new().inner_margin(Margin::symmetric(16, 8)))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    self.accounts_section(ui);
                    ui.add_space(14.0);
                    self.desktop_section(ui);
                    ui.add_space(8.0);
                });
            });

        self.remove_confirmation(&ctx);
    }
}

impl App {
    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Claude Swap").size(22.0).strong());
            ui.add_space(12.0);
            match self.active_account() {
                Some(a) => {
                    ui.label(RichText::new("active").small().color(muted(ui)));
                    ui.label(RichText::new(a.display_name()).strong().color(accent()));
                }
                None if self.loaded_once => {
                    ui.label(RichText::new("no managed account is active").color(muted(ui)));
                }
                None => {
                    ui.label(RichText::new("loading…").color(muted(ui)));
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let refresh = ui.add_enabled(self.in_flight.is_none(), egui::Button::new("Refresh"));
                if refresh.clicked() {
                    self.dispatch(Action::Refresh);
                }
                let age = self
                    .last_usage_refresh
                    .map(|t| timefmt::age(t.elapsed().as_secs_f64()))
                    .unwrap_or_else(|| "never".into());
                ui.label(RichText::new(format!("checked {age}")).small().color(muted(ui)));
            });
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let mut wanted = self.background_auto;
            let toggle = ui.add_enabled(self.in_flight.is_none(), egui::Checkbox::new(&mut wanted, "Auto-switch in the background"));
            if toggle.on_hover_text("Runs `cswap auto` as a login item. Every Claude Code session, including Xirp and VS Code, moves to the account with the most quota before the active one reaches the threshold.").changed() {
                self.dispatch(if wanted { Action::EnableBackgroundAuto } else { Action::DisableBackgroundAuto });
            }
            ui.add_space(8.0);
            ui.label(RichText::new("threshold").small().color(muted(ui)));
            let slider = ui.add(
                egui::Slider::new(&mut self.threshold_draft, 50.0..=99.0)
                    .suffix("%")
                    .fixed_decimals(0)
                    .show_value(true),
            );
            if slider.drag_stopped() || slider.lost_focus() {
                if (self.threshold_draft - self.threshold).abs() >= 0.5 && self.in_flight.is_none() {
                    self.dispatch(Action::SetThreshold(self.threshold_draft));
                }
            }
            let check = ui.add_enabled(self.in_flight.is_none(), egui::Button::new("Check now"));
            if check.on_hover_text("Run one auto-switch decision right now.").clicked() {
                self.dispatch(Action::AutoOnce { dry_run: false });
            }
        });
        if let Some(e) = &self.auto_last_event {
            ui.add(egui::Label::new(RichText::new(e.clone()).small().color(muted(ui))).truncate());
        }
    }

    fn footer(&mut self, ui: &mut egui::Ui) {
        if let Some(action) = &self.in_flight {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(action.label()).small().color(muted(ui)));
            });
        } else if let Some(n) = &self.notice {
            let color = if n.is_error { danger() } else { ok_color() };
            ui.add(egui::Label::new(RichText::new(n.text.clone()).small().color(color)).truncate());
        } else {
            ui.add_space(14.0);
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let idle = self.in_flight.is_none();
            let has_others = self.accounts.iter().any(|a| a.is_switchable() && !a.disabled);
            if ui.add_enabled(idle && has_others, egui::Button::new("Rotate")).clicked() {
                self.dispatch(Action::Rotate);
            }
            if ui.add_enabled(idle && has_others, egui::Button::new("Most quota")).clicked() {
                self.dispatch(Action::SwitchBest);
            }
            if ui.add_enabled(idle && has_others, egui::Button::new("Next not limited")).clicked() {
                self.dispatch(Action::SwitchNextAvailable);
            }
            ui.separator();
            let add = ui.add_enabled(idle, egui::Button::new("+ Add current login"));
            if add.on_hover_text("Snapshots whatever account Claude Code is logged in as right now. Run /login first to add a different one.").clicked() {
                self.dispatch(Action::AddCurrentLogin);
            }
        });
    }

    fn accounts_section(&mut self, ui: &mut egui::Ui) {
        section_title(ui, "Accounts");
        if self.accounts.is_empty() {
            let text = if self.loaded_once {
                "No accounts yet. Log in to Claude Code, then press “Add current login”."
            } else {
                "Loading accounts…"
            };
            ui.label(RichText::new(text).color(muted(ui)));
            return;
        }
        let rows = self.accounts.clone();
        for row in &rows {
            self.account_card(ui, row);
            ui.add_space(8.0);
        }
    }

    fn account_card(&mut self, ui: &mut egui::Ui, row: &AccountRow) {
        let stroke = if row.active { Stroke::new(1.5, accent()) } else { Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color) };
        Frame::new()
            .fill(card_fill(ui))
            .stroke(stroke)
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            let dot = "•";
                            ui.label(RichText::new(dot).color(if row.active { accent() } else { muted(ui) }));
                            ui.label(RichText::new(row.display_name()).size(16.0).strong());
                            if row.active {
                                badge(ui, "active", accent());
                            }
                            if row.disabled {
                                badge(ui, "held out", muted(ui));
                            }
                            if let Some(status) = row.status_label() {
                                badge(ui, status, danger());
                            }
                        });
                        ui.label(RichText::new(format!("#{} · {}", row.number, row.organization_name)).small().color(muted(ui)));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        self.card_buttons(ui, row);
                    });
                });
                ui.add_space(8.0);
                match row.shown_usage() {
                    Some((usage, fresh)) => {
                        for (label, window) in usage.windows() {
                            usage_bar(ui, &label, window);
                        }
                        if let Some(spend) = &usage.spend {
                            spend_bar(ui, spend);
                        }
                        let age = if fresh { row.usage_age_seconds } else { row.last_good_age_seconds };
                        let mut note = format!("measured {}", age.map(timefmt::age).unwrap_or_else(|| "at an unknown time".into()));
                        if !fresh {
                            note.push_str(" · too old to drive switching");
                        }
                        if let Some(retry) = &row.usage_retry_at {
                            note.push_str(&format!(" · usage API backing off until {}", timefmt::clock(retry)));
                        } else if let Some(err) = &row.usage_error {
                            note.push_str(&format!(" · {err}"));
                        }
                        ui.label(RichText::new(note).small().color(if fresh { muted(ui) } else { warn() }));
                    }
                    None => {
                        let why = row.usage_error.clone().unwrap_or_else(|| "no usage data".into());
                        ui.label(RichText::new(why).small().color(muted(ui)));
                    }
                }
                if let Some(expires) = &row.login_expires_at {
                    let left = timefmt::until(expires);
                    ui.label(RichText::new(format!("login valid for {left}")).small().color(muted(ui)));
                }
            });
    }

    fn card_buttons(&mut self, ui: &mut egui::Ui, row: &AccountRow) {
        let idle = self.in_flight.is_none();
        if ui.add_enabled(idle, egui::Button::new("Remove")).clicked() {
            self.confirm_remove = Some(row.number);
        }
        let hold_label = if row.disabled { "Rejoin rotation" } else { "Hold out" };
        if ui.add_enabled(idle, egui::Button::new(hold_label)).clicked() {
            self.dispatch(if row.disabled { Action::Enable(row.number) } else { Action::Disable(row.number) });
        }
        let term = ui.add_enabled(idle, egui::Button::new("Terminal"));
        if term.on_hover_text("Open a Terminal window running Claude Code as this account only; other terminals stay on the active account.").clicked() {
            self.dispatch(Action::OpenTerminal(row.number));
        }
        let switch = ui.add_enabled(idle && row.is_switchable(), egui::Button::new(RichText::new("Switch").strong()));
        if switch.on_hover_text("Make this the account every Claude Code session uses.").clicked() {
            self.dispatch(Action::SwitchTo(row.number));
        }
    }

    fn desktop_section(&mut self, ui: &mut egui::Ui) {
        section_title(ui, "Claude desktop app sessions");
        ui.label(
            RichText::new("The desktop app keeps its own login and cannot be switched from here. When one of its sessions hits a limit, resume it in a Terminal as another account.")
                .small()
                .color(muted(ui)),
        );
        ui.add_space(6.0);
        let sessions = self.desktop_sessions.clone();
        if sessions.is_empty() {
            ui.label(RichText::new("No desktop app sessions found.").color(muted(ui)));
            return;
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("resume as").small().color(muted(ui)));
            let current = self
                .resume_target
                .and_then(|n| self.accounts.iter().find(|a| a.number == n))
                .map(|a| a.display_name())
                .unwrap_or_else(|| "choose…".into());
            egui::ComboBox::from_id_salt("resume-target").selected_text(current).show_ui(ui, |ui| {
                for a in &self.accounts {
                    ui.selectable_value(&mut self.resume_target, Some(a.number), a.display_name());
                }
            });
        });
        ui.add_space(4.0);
        for s in &sessions {
            Frame::new()
                .fill(card_fill(ui))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let alive = s.is_alive();
                        ui.label(RichText::new("•").size(18.0).color(if alive { ok_color() } else { muted(ui) }));
                        let can = self.in_flight.is_none() && self.resume_target.is_some();
                        let mut resume = false;
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            resume = ui.add_enabled(can, egui::Button::new("Resume in Terminal")).clicked();
                            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                                ui.add(egui::Label::new(RichText::new(&s.name).strong()).truncate());
                                ui.add(egui::Label::new(RichText::new(s.short_folder()).small().color(muted(ui))).truncate());
                            });
                        });
                        if resume {
                            self.dispatch(Action::ResumeDesktopSession {
                                account: self.resume_target.unwrap(),
                                session_id: s.session_id.clone(),
                                cwd: s.cwd.clone(),
                            });
                        }
                    });
                });
            ui.add_space(6.0);
        }
    }

    fn remove_confirmation(&mut self, ctx: &egui::Context) {
        let Some(number) = self.confirm_remove else { return };
        let email = self
            .accounts
            .iter()
            .find(|a| a.number == number)
            .map(|a| a.email.clone())
            .unwrap_or_default();
        let modal = egui::Modal::new(egui::Id::new("confirm-remove")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(RichText::new("Remove account?").size(18.0).strong());
            ui.add_space(6.0);
            ui.label(format!("{email} will be removed from claude-swap. The login itself is not revoked; add it again later with “Add current login”."));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    self.confirm_remove = None;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(RichText::new("Remove").color(Color32::WHITE)).fill(danger())).clicked() {
                        self.confirm_remove = None;
                        self.dispatch(Action::Remove(number));
                    }
                });
            });
        });
        if modal.should_close() {
            self.confirm_remove = None;
        }
    }

    fn active_account(&self) -> Option<&AccountRow> {
        self.accounts.iter().find(|a| a.active)
    }
}

fn read_threshold() -> f64 {
    let value: Result<serde_json::Value, _> = cswap::run_json(&["config", "get", "autoswitch.threshold", "--json"]);
    value
        .ok()
        .and_then(|v| {
            v.get("value")
                .and_then(|x| x.as_f64())
                .or_else(|| v.as_f64())
        })
        .unwrap_or(90.0)
}

fn section_title(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(13.0).strong().color(muted(ui)));
    ui.add_space(6.0);
}

fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    Frame::new()
        .fill(color.gamma_multiply(0.18))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().color(color));
        });
}

fn usage_bar(ui: &mut egui::Ui, label: &str, window: &Window) {
    ui.horizontal(|ui| {
        ui.add_sized([96.0, 18.0], egui::Label::new(RichText::new(label).small()).halign(egui::Align::Min));
        let pct = window.pct.clamp(0.0, 100.0);
        let bar = egui::ProgressBar::new((pct / 100.0) as f32)
            .desired_width(220.0)
            .desired_height(12.0)
            .fill(level_color(pct))
            .corner_radius(CornerRadius::same(6));
        ui.add(bar);
        let mut text = format!("{pct:.0}%");
        if let Some(reset) = &window.resets_at {
            text.push_str(&format!("  · resets {} ({})", timefmt::clock(reset), timefmt::until(reset)));
        }
        if window.ahead_of_pace {
            text.push_str("  · ahead of pace");
        }
        ui.label(RichText::new(text).small().color(if pct >= 100.0 { danger() } else { ui.visuals().text_color() }));
    });
}

fn spend_bar(ui: &mut egui::Ui, spend: &Spend) {
    ui.horizontal(|ui| {
        ui.add_sized([96.0, 18.0], egui::Label::new(RichText::new("extra usage").small()).halign(egui::Align::Min));
        let pct = spend.pct.clamp(0.0, 100.0);
        ui.add(
            egui::ProgressBar::new((pct / 100.0) as f32)
                .desired_width(220.0)
                .desired_height(12.0)
                .fill(level_color(pct))
                .corner_radius(CornerRadius::same(6)),
        );
        ui.label(RichText::new(format!("{:.2} / {:.2} {}", spend.used, spend.limit, spend.currency)).small());
    });
}

fn level_color(pct: f64) -> Color32 {
    if pct >= 80.0 {
        danger()
    } else if pct >= 50.0 {
        warn()
    } else {
        ok_color()
    }
}

fn accent() -> Color32 {
    Color32::from_rgb(217, 119, 87)
}

fn ok_color() -> Color32 {
    Color32::from_rgb(76, 175, 110)
}

fn warn() -> Color32 {
    Color32::from_rgb(222, 170, 60)
}

fn danger() -> Color32 {
    Color32::from_rgb(220, 80, 80)
}

fn muted(ui: &egui::Ui) -> Color32 {
    ui.visuals().weak_text_color()
}

fn card_fill(ui: &egui::Ui) -> Color32 {
    ui.visuals().faint_bg_color
}
