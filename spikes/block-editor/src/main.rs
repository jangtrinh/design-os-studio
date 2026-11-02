//! THROWAWAY spike for issue 1526 (Notion-style block editor). Not part of the Studio workspace.
//!
//! cargo run --release --manifest-path spikes/block-editor/Cargo.toml -- [--mode custom|textedit]
//!     [--script] [--shots DIR]

mod app;
mod custom;
mod history;
mod model;
mod render;
mod script;
mod slash;
mod slash_ui;
mod tedit;
mod theme;

use app::{App, Mode};
use egui::{Context, RawInput};

struct Spike {
    app: App,
}

impl eframe::App for Spike {
    fn raw_input_hook(&mut self, ctx: &Context, raw: &mut RawInput) {
        if let Some(mut s) = self.app.script.take() {
            s.tick(&mut self.app, ctx, raw);
            self.app.script = Some(s);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let app = &mut self.app;
        app.begin_frame(&ctx);
        let (undo, redo) = ctx.input_mut(|i| {
            let redo = i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z);
            let undo = !redo && i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z);
            (undo, redo)
        });
        if undo || redo {
            app.op_undo(redo);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::P)) {
            // manual capture hotkey, used to snapshot the real-IME run
            app.shot_counter += 1;
            app.pending_shot = Some(format!("real-ime-{}", app.shot_counter));
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            println!("MARKDOWN-BEGIN\n{}MARKDOWN-END", app.markdown());
        }
        app.slash_keys(&ctx);
        match app.mode {
            Mode::Custom => custom::input(app, &ctx),
            Mode::TextEdit => tedit::pre_input(app, &ctx),
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(theme::BG)).show(ui, |ui| {
            if app.mode == Mode::Custom {
                ui.interact(ui.max_rect(), custom::editor_id(), egui::Sense::focusable_noninteractive());
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.add_space(48.0);
                for idx in 0..app.doc.blocks.len() {
                    render::row(ui, app, idx, |ui, app, w| match app.mode {
                        Mode::Custom => custom::show(ui, app, idx, w),
                        Mode::TextEdit => tedit::show(ui, app, idx, w),
                    });
                }
                ui.add_space(240.0);
            });
        });
        app.end_frame();
        app.slash_show(&ctx);
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned());
    let mode = match flag("--mode").as_deref() {
        Some("textedit") => Mode::TextEdit,
        _ => Mode::Custom,
    };
    let mut app = App::new(mode);
    app.shots_dir = flag("--shots").map(Into::into);
    app.log_ime = args.iter().any(|a| a == "--log-ime");
    if args.iter().any(|a| a == "--script") {
        app.script = Some(script::Script::new());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1180.0, 820.0]).with_title(format!("block editor spike ({mode:?})")),
        ..Default::default()
    };
    eframe::run_native("block-editor-spike", options, Box::new(move |cc| {
        theme::install(&cc.egui_ctx);
        Ok(Box::new(Spike { app }))
    }))
}
