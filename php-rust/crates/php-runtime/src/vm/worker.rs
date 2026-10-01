//! Worker mode (fork, DECISION_KERNEL.md §5): the application boots once per
//! worker, then `ferro_handle_request(callable)` serves requests in a loop on
//! the same `Vm` — the class table, loaded units, statics and every object
//! the boot script created stay alive across requests, as in FrankenPHP's
//! worker mode. The host (the `ferro -S … --worker` front end) owns the
//! socket: it installs [`php_types::sapi::WorkerHooks`] on the worker's
//! thread, and this module only asks for the next request and hands back
//! the response.
//!
//! What is reset per request and what is kept follows the table in
//! DECISION_KERNEL.md §5.3: output, headers, diagnostics, the superglobals,
//! error/exception handlers and ini values back to their post-boot snapshot,
//! shutdown functions run and cleared, the session written and closed,
//! upload temp files deleted. Kept: globals, function statics, static
//! properties, objects — and the object/resource id counters, which must
//! stay unique across the worker's life (so this is NOT `request_end`,
//! which resets them to 1).

use super::*;

/// The post-boot snapshot, taken on the first `ferro_handle_request()`.
pub(super) struct WorkerState {
    exception_handlers: Vec<Zval>,
    error_handlers: Vec<(Zval, i64)>,
    ini: ini::IniTable,
    /// Requests served by this worker so far.
    pub requests: u64,
}

impl<'m> Vm<'m> {
    /// `ferro_handle_request(callable $handler): bool` — block until the host
    /// has a request, run `$handler` for it, send the response; `false` when
    /// the host is shutting down (the worker script then returns).
    pub(super) fn ho_ferro_handle_request(&mut self, args: Vec<Zval>) -> Result<Zval, PhpError> {
        let Some(cb) = args.into_iter().next() else {
            return Err(PhpError::ArgumentCountError(
                "ferro_handle_request() expects exactly 1 argument, 0 given".to_string(),
            ));
        };
        let cb = cb.deref_clone();
        let Some(req) = php_types::sapi::worker_next_request() else {
            return Err(PhpError::Error(
                "ferro_handle_request(): not running under a worker SAPI (ferro -S --worker)"
                    .to_string(),
            ));
        };
        let Some(req) = req else {
            return Ok(Zval::Bool(false));
        };
        self.worker_request_begin(Rc::new(req));
        let result = self.call_callable(cb, Vec::new());
        self.final_flush = true;
        match result {
            Ok(_) | Err(PhpError::Exit(_)) => {}
            // An uncaught throwable or a fatal ends the REQUEST, not the
            // worker: rendered like the one-shot SAPI renders it.
            Err(e) => {
                let line = self.fatal_line;
                let _ = self.flush_diags(line);
                if !self.handle_uncaught_exception(&e) {
                    self.flush_all_output_buffers();
                    // PHP answers 500 only when the error is NOT displayed
                    // (php_error_cb: display_errors off, headers not sent).
                    if self.response_code.is_none()
                        && !self.output_started
                        && !self.ini.get_bool(b"display_errors")
                    {
                        self.response_code = Some(500);
                    }
                    self.render_fatal(&e, line);
                }
            }
        }
        let resp = self.worker_request_end();
        php_types::sapi::worker_send_response(resp);
        Ok(Zval::Bool(true))
    }

    /// Per-request setup: the first call switches the `Vm` to the web SAPI
    /// (as `request_start` does for a one-shot request) and snapshots the
    /// post-boot state; every call clears the output/diagnostic channels and
    /// seeds the superglobals from `req`.
    fn worker_request_begin(&mut self, req: Rc<php_types::sapi::WebRequest>) {
        if self.worker.is_none() {
            self.web = true;
            for (name, value) in [
                (&b"html_errors"[..], &b"1"[..]),
                (b"output_buffering", b"4096"),
                (b"implicit_flush", b""),
                (b"max_execution_time", b"30"),
                (b"max_input_time", b"60"),
            ] {
                if let Some(e) = self.ini.0.get_mut(name) {
                    e.global = value.to_vec();
                    e.local = value.to_vec();
                }
            }
            self.worker = Some(Box::new(WorkerState {
                exception_handlers: self.exception_handlers.clone(),
                error_handlers: self.error_handlers.clone(),
                ini: self.ini.clone(),
                requests: 0,
            }));
        }
        php_types::sapi::set_web_request(Rc::clone(&req));
        self.stdout.clear();
        self.rendered.clear();
        self.ob_stack.clear();
        self.diags = Diags::new();
        self.diags_rendered = 0;
        self.diag_line_marks.clear();
        self.error_log.clear();
        websapi::seed_web_superglobals(&mut self.superglobals, &req);
        self.response_headers.clear();
        self.response_headers.push(b"X-Powered-By: PHP/8.5.7".to_vec());
        self.response_code = None;
        self.response_reason = None;
        self.output_started = false;
        self.output_start = None;
        self.last_error = None;
        self.in_error_handler = false;
        self.final_flush = false;
        self.suppress_depth = 0;
        self.suppress_marks.clear();
        self.silence_saved.clear();
        self.fatal_line = 0;
    }

    /// Request shutdown without the process teardown: shutdown functions,
    /// output flush, session write, then the response is taken and the
    /// handlers / ini go back to the post-boot snapshot. Destructors are NOT
    /// forced (objects that die during the request were destructed when
    /// they died; the boot-time ones must live on) and cycles are left to
    /// the worker script's `gc_collect_cycles()`.
    fn worker_request_end(&mut self) -> php_types::sapi::WorkerResponse {
        self.run_shutdown_functions();
        self.flush_all_output_buffers();
        self.session_shutdown_flush();
        self.session = session::SessionState::default();
        self.finalize_filtered_streams();
        let resp = php_types::sapi::WorkerResponse {
            status: self.response_code.unwrap_or(200),
            reason: self.response_reason.take(),
            headers: std::mem::take(&mut self.response_headers),
            body: std::mem::take(&mut self.rendered),
            error_log: std::mem::take(&mut self.error_log),
        };
        self.stdout.clear();
        self.shutdown_fns.clear();
        if let Some(ws) = self.worker.as_mut() {
            ws.requests += 1;
            self.exception_handlers = ws.exception_handlers.clone();
            self.error_handlers = ws.error_handlers.clone();
            self.ini = ws.ini.clone();
        }
        php_types::sapi::clear_web_request();
        for tmp in php_types::sapi::take_uploaded_files() {
            use std::os::unix::ffi::OsStrExt;
            let _ = std::fs::remove_file(std::ffi::OsStr::from_bytes(&tmp));
        }
        resp
    }
}
