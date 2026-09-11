//! All GPUI state and rendering for Pitype.

use std::borrow::Cow;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::chart::AreaChart;
use gpui_kit::component::h_flex;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::v_flex;
use gpui_kit::component::{Root, Sizable as _, Theme, ThemeMode};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use pitype::engine::{classify_key, Engine, KeyInput, Mode, SessionResult};
use pitype::lessons::{PromptSpec, QUOTES};
use pitype::metrics::accuracy;
use pitype::stats::{SessionRecord, Stats, StatsStore};
use pitype::theme::{detect_system_dark, detect_text_scale, OmarchyPalette};

actions!(
    pitype_actions,
    [
        Restart,
        ToMenu,
        CycleMode,
        SelectValue,
        ToggleFullscreen,
        Quit
    ]
);

pub fn init(cx: &mut App) {
    load_fonts(cx);
    cx.bind_keys([
        KeyBinding::new("ctrl-r", Restart, None),
        KeyBinding::new("escape", ToMenu, None),
        // Enter is handled per-screen in `on_key_down`: plain start on the
        // menu, double-press confirmation on results, nothing mid-run.
        KeyBinding::new("tab", CycleMode, None),
        KeyBinding::new("shift-tab", SelectValue, None),
        KeyBinding::new("f11", ToggleFullscreen, None),
        KeyBinding::new("super-f", ToggleFullscreen, None),
        KeyBinding::new("ctrl-q", Quit, None),
    ]);
}

fn load_fonts(cx: &mut App) {
    let fonts: [&'static [u8]; 4] = [
        include_bytes!("../fonts/iAWriterMonoS-Regular.ttf"),
        include_bytes!("../fonts/iAWriterMonoS-Italic.ttf"),
        include_bytes!("../fonts/iAWriterMonoS-Bold.ttf"),
        include_bytes!("../fonts/iAWriterMonoS-BoldItalic.ttf"),
    ];
    let blobs = fonts.into_iter().map(Cow::Borrowed).collect::<Vec<_>>();
    let _ = cx.text_system().add_fonts(blobs);
}

pub fn open_window(cx: &mut AsyncApp) -> anyhow::Result<WindowHandle<Root>> {
    cx.open_window(window_options(), |window, cx| {
        let view = cx.new(|cx| Pitype::new(window, cx));
        cx.new(|cx| Root::new(view, window, cx))
    })
    .map_err(|e| anyhow::anyhow!("{e}"))
}

fn window_options() -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(80.), px(60.)),
            size: size(px(920.), px(620.)),
        })),
        window_min_size: Some(size(px(640.), px(480.))),
        titlebar: Some(TitlebarOptions {
            title: Some("Pitype".into()),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        app_id: Some("pitype".into()),
        ..Default::default()
    }
}

pub fn app_paths() -> (PathBuf, PathBuf) {
    let base = directories::ProjectDirs::from("dev", "pitype", "pitype")
        .map(|p| p.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".").join("pitype-data"));
    let _ = std::fs::create_dir_all(&base);
    (base.clone(), base.join("stats.json"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Menu,
    Typing,
    Results,
}

pub struct Pitype {
    focus_handle: FocusHandle,
    screen: Screen,
    mode: Mode,
    /// Index into the mode's choice list (Time seconds, Words count, Quote).
    mode_index: usize,
    spec: PromptSpec,
    engine: Option<Engine>,
    last_result: Option<SessionResult>,
    last_record: Option<SessionRecord>,
    new_best: bool,
    palette: OmarchyPalette,
    text_scale: f32,
    stats: Stats,
    stats_store: StatsStore,
    /// First Enter press on the results screen, awaiting confirmation.
    confirm_at: Option<Instant>,
    _poll_task: Option<Task<()>>,
    _confirm_task: Option<Task<()>>,
}

/// How long the first Enter press on results stays armed for the second.
const ENTER_CONFIRM: Duration = Duration::from_secs(2);

impl Pitype {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dark = detect_system_dark();
        let palette = OmarchyPalette::load(dark);
        let text_scale = detect_text_scale();
        let (_data_dir, stats_path) = app_paths();
        let stats_store = StatsStore::new(stats_path);
        let stats = stats_store.load();

        let focus_handle = cx.focus_handle();
        let initial_focus = focus_handle.clone();
        window.defer(cx, move |window, cx| {
            window.focus(&initial_focus, cx);
        });

        let mut this = Self {
            focus_handle,
            screen: Screen::Menu,
            mode: Mode::Timed,
            mode_index: 1,
            spec: PromptSpec::Time(Duration::from_secs(30)),
            engine: None,
            last_result: None,
            last_record: None,
            new_best: false,
            palette,
            text_scale,
            stats,
            stats_store,
            confirm_at: None,
            _poll_task: None,
            _confirm_task: None,
        };
        this.spec = this.current_spec();
        this.spawn_poll(window, cx);
        this
    }

    fn spawn_poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _appearance = cx.observe_window_appearance(window, |this, window, cx| {
            this.poll_theme(window, cx);
        });
        let poll = cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(400))
                .await;
            if this
                .update_in(cx, |this, window, cx| this.poll_theme(window, cx))
                .is_err()
            {
                break;
            }
        });
        self._poll_task = Some(poll);
    }

    /// Every 400 ms: re-read the Omarchy palette, re-apply when it changed.
    fn poll_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dark = detect_system_dark();
        let fresh = OmarchyPalette::load(dark);
        if fresh != self.palette {
            self.palette = fresh;
            self.apply_palette(window, cx);
            cx.notify();
        }
        if self.screen == Screen::Typing {
            self.tick_typing(window, cx);
        }
    }

    fn tick_typing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        let now = Instant::now();
        engine.tick_second(now);
        if engine.check_finished(&self.spec, now) {
            self.finish_run(window, cx);
            return;
        }
        cx.notify();
    }

    fn scaled(&self, value: f32) -> f32 {
        value * self.text_scale
    }

    fn current_spec(&self) -> PromptSpec {
        match self.mode {
            Mode::Timed => PromptSpec::Time(Duration::from_secs(
                PromptSpec::TIME_CHOICES[self.mode_index.min(3)],
            )),
            Mode::Words => PromptSpec::Words(PromptSpec::WORD_CHOICES[self.mode_index.min(3)]),
            Mode::Quote => PromptSpec::Quote(self.mode_index % PromptSpec::QUOTE_COUNT),
        }
    }

    fn start_run(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.spec = self.current_spec();
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_millis() as u64 ^ d.as_secs())
            .unwrap_or(0);
        let prompt = pitype::lessons::build_prompt(&self.spec, seed);
        self.engine = Some(Engine::new(prompt));
        self.last_result = None;
        self.last_record = None;
        self.new_best = false;
        self.screen = Screen::Typing;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    fn show_menu(&mut self, cx: &mut Context<Self>) {
        self.screen = Screen::Menu;
        self.engine = None;
        self.confirm_at = None;
        self.stats = self.stats_store.load();
        cx.notify();
    }

    fn finish_run(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = Instant::now();
        let Some(engine) = self.engine.as_ref() else {
            return;
        };
        let result = engine.finish(&self.spec, now);
        // No finish path today can produce a zero-keystroke run (the clock
        // starts on the first printable key); guard anyway so a future
        // finish path can't persist an empty session.
        if engine.keystrokes() == 0 {
            self.show_menu(cx);
            return;
        }
        let record = SessionRecord {
            finished_at: chrono::Utc::now().timestamp(),
            mode_tag: result.mode_tag.clone(),
            prompt_chars: result.prompt_chars,
            duration_s: result.duration_s,
            net_wpm: result.net_wpm,
            raw_wpm: result.raw_wpm,
            accuracy: result.accuracy,
            errors: result.errors,
            consistency: result.consistency,
        };
        let (stats, new_best) = self.stats_store.record(self.stats.clone(), record.clone());
        self.stats = stats;
        self.last_record = Some(record);
        self.last_result = Some(result);
        self.new_best = new_best;
        self.screen = Screen::Results;
        self.confirm_at = None;
        let _ = window;
        cx.notify();
    }

    fn apply_palette(&self, window: &mut Window, cx: &mut Context<Self>) {
        let mode = if self.palette.dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        Theme::change(mode, Some(window), cx);
        let theme = Theme::global_mut(cx);
        theme.colors.background = hex_to_hsla(&self.palette.background);
        theme.colors.foreground = hex_to_hsla(&self.palette.foreground);
        theme.colors.primary = hex_to_hsla(&self.palette.accent);
        theme.colors.primary_hover = hex_to_hsla(&self.palette.accent);
        theme.colors.secondary = hex_to_hsla(&self.palette.surface);
        theme.colors.secondary_hover = hex_to_hsla(&self.palette.hover);
        theme.mono_font_family = "iA Writer Mono S".into();
        theme.font_family = "iA Writer Mono S".into();
        theme.mono_font_size = px(self.scaled(20.));
        theme.font_size = px(self.scaled(16.));
        Theme::sync_base(cx);
    }
}

impl Focusable for Pitype {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn hex_to_hsla(value: &str) -> Hsla {
    parse(value).unwrap_or(rgb(0x909191).into())
}

fn parse(value: &str) -> Option<Hsla> {
    let hex = value.trim().trim_start_matches('#');
    let expanded = if hex.len() == 3 {
        hex.chars().flat_map(|c| [c, c]).collect::<String>()
    } else {
        hex.to_string()
    };
    let n = u32::from_str_radix(&expanded, 16).ok()?;
    Some(rgb(n).into())
}

impl Render for Pitype {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.apply_palette(window, cx);
        let fg = hex_to_hsla(&self.palette.foreground);
        let muted = hex_to_hsla(&self.palette.muted);
        let accent = hex_to_hsla(&self.palette.accent);
        let surface = hex_to_hsla(&self.palette.surface);

        v_flex()
            .id("pitype")
            .key_context("pitype")
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(hex_to_hsla(&self.palette.background))
            .text_color(fg)
            .font_family("iA Writer Mono S")
            .on_action(cx.listener(|this, _: &Restart, window, cx| this.restart(window, cx)))
            .on_action(cx.listener(|this, _: &ToMenu, _, cx| this.show_menu(cx)))
            .on_action(cx.listener(|this, _: &CycleMode, _, cx| this.cycle_mode(cx)))
            .on_action(cx.listener(|this, _: &SelectValue, _, cx| this.next_value(cx)))
            .on_action(
                cx.listener(|_: &mut Self, _: &ToggleFullscreen, window, _| {
                    window.toggle_fullscreen();
                }),
            )
            .on_action(cx.listener(|_: &mut Self, _: &Quit, _, cx| cx.quit()))
            .on_key_down(cx.listener(Self::on_key_down))
            .when(self.screen == Screen::Menu, |this| {
                this.child(self.render_menu(muted, accent, surface, cx))
            })
            .when(self.screen == Screen::Typing, |this| {
                this.child(self.render_typing(muted, accent, cx))
            })
            .when(self.screen == Screen::Results, |this| {
                this.child(self.render_results(muted, accent, cx))
            })
    }
}

impl Pitype {
    fn restart(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.start_run(window, cx);
    }

    fn cycle_mode(&mut self, cx: &mut Context<Self>) {
        if self.screen != Screen::Menu {
            return;
        }
        self.mode = self.mode.next();
        self.mode_index = 0;
        self.spec = self.current_spec();
        cx.notify();
    }

    fn next_value(&mut self, cx: &mut Context<Self>) {
        if self.screen != Screen::Menu {
            return;
        }
        let choices = match self.mode {
            Mode::Timed => PromptSpec::TIME_CHOICES.len(),
            Mode::Words => PromptSpec::WORD_CHOICES.len(),
            Mode::Quote => PromptSpec::QUOTE_COUNT,
        };
        self.mode_index = (self.mode_index + 1) % choices;
        self.spec = self.current_spec();
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        // Modifiers and function keys stay with the bound actions.
        if keystroke.modifiers.control
            || keystroke.modifiers.alt
            || keystroke.modifiers.platform
            || keystroke.modifiers.function
        {
            return;
        }
        match self.screen {
            Screen::Menu => {
                // Enter or any printable key starts, as the hint promises;
                // other keys (backspace, arrows, …) do nothing.
                if keystroke.key == "enter" {
                    self.start_run(window, cx);
                    return;
                }
                if matches!(
                    classify_key(&keystroke.key, keystroke.key_char.as_deref()),
                    KeyInput::Char(_)
                ) {
                    self.start_run(window, cx);
                }
            }
            Screen::Typing => {
                let Some(engine) = self.engine.as_mut() else {
                    return;
                };
                let key = classify_key(&keystroke.key, keystroke.key_char.as_deref());
                if key == KeyInput::Ignored {
                    return;
                }
                engine.type_key(key, Instant::now());
                engine.tick_second(Instant::now());
                if engine.check_finished(&self.spec, Instant::now()) {
                    self.finish_run(window, cx);
                    return;
                }
                cx.notify();
            }
            Screen::Results => {
                if keystroke.key != "enter" {
                    return;
                }
                // Double Enter starts the next run; a lone press just arms
                // the hint and expires after a moment.
                let now = Instant::now();
                let armed = self
                    .confirm_at
                    .is_some_and(|at| now.duration_since(at) <= ENTER_CONFIRM);
                if armed {
                    self.confirm_at = None;
                    self.start_run(window, cx);
                } else {
                    self.confirm_at = Some(now);
                    self.spawn_confirm_reset(window, cx);
                    cx.notify();
                }
            }
        }
    }

    /// Clear the armed Enter hint once the confirmation window passes.
    fn spawn_confirm_reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let task = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(ENTER_CONFIRM).await;
            let _ = this.update_in(cx, |this, _, cx| {
                this.confirm_at = None;
                cx.notify();
            });
        });
        self._confirm_task = Some(task);
    }

    // ---- Menu -----------------------------------------------------------

    fn render_menu(
        &self,
        muted: Hsla,
        accent: Hsla,
        surface: Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let scale = self.text_scale;
        let tag = self.current_spec().mode_tag();
        let best = self
            .stats
            .bests
            .get(&tag)
            .map(|w| format!("Best: {:.1} wpm", w))
            .unwrap_or_else(|| "No best yet — start typing!".into());

        let mode_tabs = TabBar::new("mode-tabs")
            .segmented()
            .selected_index(match self.mode {
                Mode::Timed => 0,
                Mode::Words => 1,
                Mode::Quote => 2,
            })
            .on_click(cx.listener(|this, index: &usize, _, cx| {
                this.mode = match index {
                    0 => Mode::Timed,
                    1 => Mode::Words,
                    _ => Mode::Quote,
                };
                this.mode_index = this.mode_index.min(3);
                this.spec = this.current_spec();
                cx.notify();
            }))
            .child(Tab::new().label("Time"))
            .child(Tab::new().label("Words"))
            .child(Tab::new().label("Quote"));

        let choices = match self.mode {
            Mode::Timed => PromptSpec::TIME_CHOICES
                .iter()
                .map(|s| format!("{s}s"))
                .collect::<Vec<_>>(),
            Mode::Words => PromptSpec::WORD_CHOICES
                .iter()
                .map(|w| format!("{w} words"))
                .collect::<Vec<_>>(),
            // Quote mode renders its list below instead of buttons.
            Mode::Quote => Vec::new(),
        };

        let choice_area = match self.mode {
            Mode::Quote => self.render_quote_list(muted, accent, surface, cx),
            _ => self.render_value_row(&choices, cx),
        };

        v_flex()
            .id("menu")
            .flex_1()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                h_flex()
                    .items_baseline()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(scale * 44.))
                            .text_color(accent)
                            .child("pitype"),
                    )
                    .child(
                        div()
                            .text_size(px(scale * 14.))
                            .text_color(muted)
                            .child("a dead-simple typing practice app"),
                    ),
            )
            .child(div().w(px(420. * scale)).child(mode_tabs))
            .child(choice_area)
            .child(
                div()
                    .text_size(px(scale * 13.))
                    .text_color(muted)
                    .child(best),
            )
            .child(
                div()
                    .id("start-hint")
                    .rounded_md()
                    .px(px(scale * 16.))
                    .py(px(scale * 8.))
                    .bg(surface)
                    .text_size(px(scale * 15.))
                    .child("start typing to begin")
                    .with_animation(
                        "start-hint-fade",
                        Animation::new(Duration::from_millis(900)).with_easing(ease_in_out),
                        |this, delta| this.opacity(delta),
                    ),
            )
            .child(self.render_status_bar(muted, cx))
            .into_any_element()
    }
}

impl Pitype {
    fn render_value_row(&self, choices: &[String], cx: &mut Context<Self>) -> AnyElement {
        let scale = self.text_scale;
        let accent = hex_to_hsla(&self.palette.accent);
        let surface = hex_to_hsla(&self.palette.surface);
        h_flex()
            .flex_wrap()
            .justify_center()
            .gap_2()
            .children(choices.iter().enumerate().map(|(i, label)| {
                let selected = self.mode_index == i;
                self.menu_card(
                    SharedString::from(format!("choice-{i}")),
                    selected,
                    accent,
                    surface,
                )
                .flex()
                .items_center()
                .justify_center()
                .min_w(px(scale * 96.))
                .text_color(if selected {
                    accent
                } else {
                    fg_color(&self.palette)
                })
                .child(label.clone())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.mode_index = i;
                    this.spec = this.current_spec();
                    cx.notify();
                }))
            }))
            .into_any_element()
    }

    /// The one card treatment shared by every menu picker (Time values,
    /// Words values, quote rows).
    fn menu_card(
        &self,
        id: SharedString,
        selected: bool,
        accent: Hsla,
        surface: Hsla,
    ) -> Stateful<Div> {
        let scale = self.text_scale;
        div()
            .id(id)
            .px(px(scale * 12.))
            .py(px(scale * 7.))
            .rounded_md()
            .text_size(px(scale * 13.))
            .bg(if selected {
                accent.opacity(0.16)
            } else {
                surface.opacity(0.5)
            })
            .hover(move |s| {
                s.bg(if selected {
                    accent.opacity(0.24)
                } else {
                    surface
                })
            })
    }

    /// Quote picker: one row per quote with index, full author, and a
    /// text preview — same card style as the value pickers.
    fn render_quote_list(
        &self,
        muted: Hsla,
        accent: Hsla,
        surface: Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let scale = self.text_scale;
        v_flex()
            .id("quote-list")
            .w(px(640. * scale))
            .max_h(px(300. * scale))
            .overflow_y_scroll()
            .gap_1()
            .children(QUOTES.iter().enumerate().map(|(i, quote)| {
                let selected = self.mode_index % QUOTES.len() == i;
                self.menu_card(
                    SharedString::from(format!("quote-{i}")),
                    selected,
                    accent,
                    surface,
                )
                .w_full()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .text_size(px(scale * 12.))
                        .text_color(if selected { accent } else { muted })
                        .child(format!("{}", i + 1)),
                )
                .child(
                    div()
                        .w(px(180. * scale))
                        .truncate()
                        .text_size(px(scale * 13.))
                        .text_color(if selected {
                            accent
                        } else {
                            fg_color(&self.palette)
                        })
                        .child(quote.author.to_string()),
                )
                .child(
                    div()
                        .flex_1()
                        .truncate()
                        .text_size(px(scale * 12.))
                        .text_color(muted)
                        .child(quote.text.to_string()),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.mode_index = i;
                    this.spec = this.current_spec();
                    cx.notify();
                }))
            }))
            .into_any_element()
    }

    // ---- Typing ---------------------------------------------------------

    fn render_typing(&self, muted: Hsla, accent: Hsla, _cx: &mut Context<Self>) -> AnyElement {
        let now = Instant::now();
        let scale = self.text_scale;
        let Some(engine) = self.engine.as_ref() else {
            return div().into_any_element();
        };
        let prompt = engine.prompt();
        let cursor = engine.cursor();
        let flash = engine.flash(now);
        let error_color = hex_to_hsla("#ff6b6b");

        // One element per word (plus spaces), not one per character, so a
        // keystroke rebuilds a handful of word nodes instead of ~1.5k char
        // nodes on long prompts.
        let mut typed_row = h_flex().flex_wrap().max_w(px(760. * scale));
        let mut word = h_flex();
        let mut char_index = 0usize;
        for ch in prompt.chars() {
            if ch == ' ' {
                typed_row = typed_row
                    .child(word)
                    .child(space_cell(muted, char_index, scale));
                word = h_flex();
                char_index += 1;
                continue;
            }
            let i = char_index;
            let is_past = i < cursor;
            let is_current = i == cursor;
            let is_error = flash == Some(i);
            let color = if is_error {
                error_color
            } else if is_past {
                fg_color(&self.palette)
            } else {
                muted
            };
            let base = div()
                .text_size(px(scale * 22.))
                .line_height(relative(1.5))
                .text_color(color)
                .when(is_current, |c| c.bg(accent.opacity(0.22)).rounded_sm())
                .when(is_error, |c| c.bg(error_color.opacity(0.18)))
                .child(ch.to_string());
            let cell = if is_current {
                base.with_animation(
                    "caret",
                    Animation::new(Duration::from_millis(900))
                        .repeat()
                        .with_easing(pulsating_between(0.35, 1.0)),
                    |this, delta| this.opacity(delta),
                )
                .into_any_element()
            } else {
                base.into_any_element()
            };
            word = word.child(cell);
            char_index += 1;
        }
        typed_row = typed_row.child(word);

        let elapsed = engine.elapsed(now);
        let wpm = engine.rolling_wpm();
        let acc = accuracy(engine.correct(), engine.errors());

        let stats_row = h_flex()
            .gap(px(scale * 24.))
            .child(stat_block("wpm", format!("{:.0}", wpm), accent, scale))
            .child(stat_block(
                "acc",
                format!("{:.1}%", acc * 100.0),
                fg_color(&self.palette),
                scale,
            ))
            .child(stat_block(
                match &self.spec {
                    PromptSpec::Time(_) => "time",
                    PromptSpec::Words(_) => "words",
                    PromptSpec::Quote(_) => "chars",
                },
                match &self.spec {
                    PromptSpec::Time(_) => format!(
                        "{:.0}s",
                        (d_secs(&self.spec) - elapsed.as_secs_f32()).max(0.0)
                    ),
                    PromptSpec::Words(n) => {
                        format!("{}/{}", engine.words_done(), n)
                    }
                    // Quote runs progress by characters, not words.
                    PromptSpec::Quote(_) => {
                        format!("{}/{}", engine.cursor(), engine.prompt().chars().count())
                    }
                },
                muted,
                scale,
            ))
            .child(stat_block(
                "err",
                format!("{}", engine.errors()),
                muted,
                scale,
            ));

        v_flex()
            .id("typing")
            .flex_1()
            .size_full()
            .items_center()
            .pt(px(scale * 48.))
            .gap_6()
            .child(typed_row)
            .when(matches!(self.spec, PromptSpec::Quote(_)), |row| {
                let index = match self.spec {
                    PromptSpec::Quote(i) => i % QUOTES.len(),
                    _ => 0,
                };
                row.child(
                    div()
                        .text_size(px(scale * 13.))
                        .text_color(muted)
                        .child(format!("— {}", QUOTES[index].author)),
                )
            })
            .child(div().h(px(1.0)))
            .child(stats_row)
            .child(
                div()
                    .text_size(px(scale * 11.))
                    .text_color(muted)
                    .child("ctrl-r restart · escape menu"),
            )
            .into_any_element()
    }

    // ---- Results --------------------------------------------------------

    fn render_results(&self, muted: Hsla, accent: Hsla, _cx: &mut Context<Self>) -> AnyElement {
        let scale = self.text_scale;
        let Some(result) = self.last_result.as_ref() else {
            return div().into_any_element();
        };
        let points = self
            .engine
            .as_ref()
            .map(|e| e.series_points())
            .unwrap_or_default();

        let mut spark = v_flex()
            .w(px(560. * scale))
            .h(px(120. * scale))
            .rounded_lg()
            .bg(hex_to_hsla(&self.palette.surface))
            .items_center()
            .justify_center();
        if points.len() >= 2 {
            let data: Vec<(usize, f64)> = points
                .iter()
                .enumerate()
                .map(|(i, w)| (i, *w as f64))
                .collect();
            spark = spark.child(
                div().size_full().child(
                    AreaChart::new(data)
                        .x(|(i, _): &(usize, f64)| SharedString::from(format!("{i}")))
                        .y(|(_, w): &(usize, f64)| *w)
                        .stroke(accent)
                        .fill(Background::from(accent.opacity(0.12)))
                        .grid(false)
                        .x_axis(false),
                ),
            );
        } else {
            spark = spark.child(
                div()
                    .text_size(px(scale * 12.))
                    .text_color(muted)
                    .child("run was too short for a chart"),
            );
        }

        let best_note = if self.new_best {
            "New best!".into()
        } else {
            self.stats
                .bests
                .get(&result.mode_tag)
                .map(|best| format!("Best: {:.1}", best))
                .unwrap_or_default()
        };

        v_flex()
            .id("results")
            .flex_1()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                h_flex()
                    .items_baseline()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(scale * 56.))
                            .text_color(accent)
                            .child(format!("{:.0}", result.net_wpm)),
                    )
                    .child(
                        div()
                            .text_size(px(scale * 14.))
                            .text_color(muted)
                            .child("wpm"),
                    ),
            )
            .child(
                h_flex()
                    .gap(px(scale * 24.))
                    .child(stat_block(
                        "acc",
                        format!("{:.1}%", result.accuracy * 100.0),
                        fg_color(&self.palette),
                        scale,
                    ))
                    .child(stat_block(
                        "raw",
                        format!("{:.0}", result.raw_wpm),
                        muted,
                        scale,
                    ))
                    .child(stat_block(
                        "consistency",
                        format!("{:.0}%", result.consistency * 100.0),
                        muted,
                        scale,
                    ))
                    .child(stat_block(
                        "errors",
                        format!("{}", result.errors),
                        muted,
                        scale,
                    )),
            )
            .child(spark)
            .child(
                div()
                    .text_size(px(scale * 13.))
                    .text_color(if self.new_best { accent } else { muted })
                    .child(best_note),
            )
            .child(
                div()
                    .text_size(px(scale * 11.))
                    .text_color(if self.confirm_at.is_some() {
                        accent
                    } else {
                        muted
                    })
                    .child(if self.confirm_at.is_some() {
                        "press enter again to go again"
                    } else {
                        "enter twice to go again · escape menu"
                    }),
            )
            .with_animation(
                "results-fade",
                Animation::new(Duration::from_millis(200)).with_easing(ease_in_out),
                |this, delta| this.opacity(delta),
            )
            .into_any_element()
    }

    // ---- Shared ---------------------------------------------------------

    fn render_status_bar(&self, muted: Hsla, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .items_center()
            .px(px(self.scaled(12.)))
            .pb(px(self.scaled(10.)))
            .gap(px(self.scaled(12.)))
            .child(
                Button::new("start")
                    .ghost()
                    .xsmall()
                    .label("Start")
                    .on_click(cx.listener(|this, _, window, cx| this.start_run(window, cx))),
            )
            .child(
                div()
                    .text_size(px(self.scaled(11.)))
                    .text_color(muted)
                    .child(format!("mode: {}", self.mode.label())),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_size(px(self.scaled(11.)))
                    .text_color(muted)
                    .child("start typing · ctrl-r restart · ctrl-q quit"),
            )
    }
}

/// A space between words in the typing view: a fixed-width cell showing a
/// faint middle dot so word boundaries stay visible while typing.
fn space_cell(muted: Hsla, index: usize, scale: f32) -> Div {
    div()
        .text_size(px(scale * 22.))
        .line_height(relative(1.5))
        .text_color(muted.opacity(0.6))
        .when(index > 0, |c| c.ml(px(0.62 * 12. * scale)))
        .child("\u{00b7}")
}

fn stat_block(label: &'static str, value: String, color: Hsla, scale: f32) -> Div {
    v_flex()
        .items_center()
        .gap_1()
        .child(
            div()
                .text_size(px(scale * 26.))
                .text_color(color)
                .child(value),
        )
        .child(div().text_size(px(scale * 11.)).child(label))
}

fn fg_color(palette: &OmarchyPalette) -> Hsla {
    hex_to_hsla(&palette.foreground)
}

fn d_secs(spec: &PromptSpec) -> f32 {
    match spec {
        PromptSpec::Time(d) => d.as_secs_f32(),
        _ => 0.0,
    }
}
