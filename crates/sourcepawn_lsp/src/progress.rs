use crate::GlobalState;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Progress {
    Begin,
    Report,
    End,
}

impl Progress {
    pub(crate) fn fraction(done: usize, total: usize) -> f64 {
        assert!(done <= total);
        done as f64 / total.max(1) as f64
    }
}

impl GlobalState {
    pub(crate) fn report_progress(
        &mut self,
        title: &str,
        state: Progress,
        message: Option<String>,
        fraction: Option<f64>,
        cancel_token: Option<String>,
    ) {
        // TODO: Implement this?
        // if !self.config.work_done_progress() {
        //     return;
        // }
        let percentage = fraction.map(|f| {
            assert!((0.0..=1.0).contains(&f));
            (f * 100.0) as u32
        });
        let cancellable = Some(cancel_token.is_some());
        let token = lsp_types::ProgressToken::String(
            cancel_token.unwrap_or_else(|| format!("rustAnalyzer/{title}")),
        );
        let work_done_progress = match state {
            Progress::Begin => {
                self.send_request::<lsp_types::request::WorkDoneProgressCreate>(
                    lsp_types::WorkDoneProgressCreateParams {
                        token: token.clone(),
                    },
                    |_, _| (),
                );

                lsp_types::WorkDoneProgress::Begin(lsp_types::WorkDoneProgressBegin {
                    title: title.into(),
                    cancellable,
                    message,
                    percentage,
                })
            }
            Progress::Report => {
                lsp_types::WorkDoneProgress::Report(lsp_types::WorkDoneProgressReport {
                    cancellable,
                    message,
                    percentage,
                })
            }
            Progress::End => {
                lsp_types::WorkDoneProgress::End(lsp_types::WorkDoneProgressEnd { message })
            }
        };
        self.send_notification::<lsp_types::notification::Progress>(lsp_types::ProgressParams {
            token,
            value: lsp_types::ProgressParamsValue::WorkDone(work_done_progress),
        });
    }
}
