//! Work that holds the window for a while, run on the frame after its status line was painted.
//!
//! egui paints at the end of a frame, so a status set inside a blocking call never shows: the window freezes with
//! the old picture. A job that can take a minute (loading every page of a large `.fig` before it is stored) is
//! queued here instead: the status goes up and a repaint is asked for, the frame paints it, and the next frame
//! starts by running the job. Only jobs nothing chains on are queued: a caller that continues with the result
//! (the questions before quitting or opening another file) runs its write directly.

use crate::app::App;

/// A job that runs on a later frame, on the app.
pub(crate) type QueuedWork = Box<dyn FnOnce(&mut App)>;

impl App {
    /// Shows `status` now and runs `work` on the next frame, after the status has been painted.
    pub(crate) fn run_after_status_paint(&mut self, status: impl Into<String>, work: impl FnOnce(&mut App) + 'static) {
        self.show_status_toast(status);
        self.queued_work.push(Box::new(work));
    }

    /// Whether a job waits for the next frame (the shell then asks for that frame).
    pub(crate) fn has_queued_work(&self) -> bool {
        !self.queued_work.is_empty()
    }

    /// Runs the jobs that earlier frames queued. Called at the start of a frame, so the status they announced is on screen.
    pub(crate) fn run_queued_work(&mut self) {
        for work in std::mem::take(&mut self.queued_work) {
            work(self);
        }
    }

    /// A quit does not drop a job the user started and whose status is on screen (an import, a Save to library): it
    /// finishes before the window closes. Returns how many jobs ran.
    pub(crate) fn finish_queued_work_before_quit(&mut self) -> usize {
        let mut queued = 0;
        while self.has_queued_work() {
            queued += self.queued_work.len();
            self.run_queued_work();
        }
        queued
    }
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use designos_doc::Document;
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn a_quit_runs_the_job_that_was_waiting_for_the_next_frame() {
        let mut app = App::new(Document::new("Doc"));
        let ran = Rc::new(Cell::new(false));
        let flag = Rc::clone(&ran);
        app.run_after_status_paint("Importing kit.fig\u{2026} loading all pages", move |_| flag.set(true));
        assert!(app.has_queued_work() && !ran.get(), "precondition: the job waits");

        assert_eq!(app.finish_queued_work_before_quit(), 1);

        assert!(ran.get() && !app.has_queued_work(), "the job finished before the window closed");
    }

    #[test]
    fn a_quit_drains_work_queued_by_another_job_in_the_same_shutdown() {
        let mut app = App::new(Document::new("Doc"));
        let ran = Rc::new(Cell::new(0));
        let first = Rc::clone(&ran);
        app.run_after_status_paint("first", move |app| {
            first.set(first.get() + 1);
            let second = Rc::clone(&first);
            app.run_after_status_paint("second", move |_| second.set(second.get() + 1));
        });

        assert_eq!(app.finish_queued_work_before_quit(), 2);
        assert_eq!(ran.get(), 2, "shutdown must finish work queued during the drain");
        assert!(!app.has_queued_work());
    }

    #[test]
    fn a_quit_with_nothing_queued_runs_nothing() {
        let mut app = App::new(Document::new("Doc"));
        assert_eq!(app.finish_queued_work_before_quit(), 0);
    }
}
