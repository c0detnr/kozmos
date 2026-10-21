use crate::{mcp, workspace};
use eframe::egui::{self, Color32, RichText};
use std::{path::Path, sync::mpsc, time::Duration};

struct Row {
    server: mcp::Server,
    report: Option<mcp::Report>,
}
struct Job {
    index: usize,
    receiver: mpsc::Receiver<mcp::Report>,
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Job {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
#[derive(Default)]
pub struct Panel {
    open: bool,
    path: String,
    rows: Vec<Row>,
    error: Option<String>,
    job: Option<Job>,
}
impl Panel {
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }
    pub fn open(&mut self, path: &Path, cwd: &Path) {
        if self.busy() {
            self.open = true;
            return;
        }
        self.path = path.to_string_lossy().into_owned();
        self.open = true;
        self.rows.clear();
        self.error = None;
        match workspace::check_path(cwd, path)
            .and_then(|()| workspace::read_document(path))
            .and_then(|raw| mcp::servers(path, &raw, cwd))
        {
            Ok(servers) => {
                self.rows = servers
                    .into_iter()
                    .map(|server| Row {
                        server,
                        report: None,
                    })
                    .collect()
            }
            Err(_) => self.error = Some(
                "Yapılandırma okunamadı. Dosya erişimini ve JSON/TOML sözdizimini kontrol edin."
                    .into(),
            ),
        }
    }
    fn start(&mut self, index: usize, ctx: &egui::Context) {
        let server = self.rows[index].server.clone();
        let (sender, receiver) = mpsc::channel();
        let (cancel, cancelled) = tokio::sync::oneshot::channel();
        let ctx = ctx.clone();
        self.rows[index].report = None;
        match std::thread::Builder::new()
            .name("mcp-test".into())
            .spawn(move || {
                let report = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime.block_on(mcp::probe(server, cancelled, mcp::TIMEOUT)),
                    Err(_) => mcp::Report {
                        elapsed: Duration::ZERO,
                        outcome: Err("Test çalışma ortamı oluşturulamadı.".into()),
                    },
                };
                let _ = sender.send(report);
                ctx.request_repaint();
            }) {
            Ok(thread) => {
                self.job = Some(Job {
                    index,
                    receiver,
                    cancel: Some(cancel),
                    thread: Some(thread),
                })
            }
            Err(_) => {
                self.rows[index].report = Some(mcp::Report {
                    elapsed: Duration::ZERO,
                    outcome: Err("Test işçisi başlatılamadı.".into()),
                })
            }
        }
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        if let Some(job) = &self.job {
            let report = match job.receiver.try_recv() {
                Ok(report) => Some(report),
                Err(mpsc::TryRecvError::Disconnected) => Some(mcp::Report {
                    elapsed: Duration::ZERO,
                    outcome: Err("Test işçisi beklenmedik şekilde kapandı.".into()),
                }),
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(report) = report {
                self.rows[job.index].report = Some(report);
                self.job = None;
            }
        }
        let mut open = self.open;
        let mut start = None;
        egui::Window::new("MCP bağlantı testi").id(egui::Id::new("mcp_diagnostics"))
            .open(&mut open).default_width(650.0).resizable(true).vscroll(true).show(ctx, |ui| {
                ui.label(RichText::new(&self.path).small().monospace());
                ui.label("Test et: yerel sunucu komutunu başlatır veya HTTP sunucusuna bağlanır. MCP el sıkışması yapar ve araçları listeler. Süre sınırı 20 saniye.");
                ui.label(RichText::new("Pencere açıldığında diskten okunan yapılandırma kullanılır. Pencereyi kapatmak testi iptal eder. Test sonunda bağlantı ve başlatılan süreçler kapatılır.").small().weak());
                ui.separator();
                if let Some(error) = &self.error { ui.colored_label(Color32::LIGHT_RED, error); }
                if self.rows.is_empty() && self.error.is_none() { ui.label("Bu yapılandırmada MCP sunucusu bulunamadı."); }
                for (i, row) in self.rows.iter().enumerate() {
                    ui.push_id(i, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(&row.server.name);
                            ui.label(row.server.transport_label());
                            if !row.server.enabled { ui.label("Devre dışı"); }
                            let running = self.job.as_ref().is_some_and(|j| j.index == i);
                            if running {
                                ui.spinner(); ui.label("Test ediliyor…");
                                if ui.button("İptal").clicked() && let Some(job) = &mut self.job && let Some(cancel) = job.cancel.take() { let _ = cancel.send(()); }
                            } else if ui.add_enabled(self.job.is_none() && row.server.enabled, egui::Button::new("Test et")).clicked() { start = Some(i); }
                        });
                        if let Some(report) = &row.report {
                            match &report.outcome {
                                Ok(details) => {
                                    ui.colored_label(Color32::LIGHT_GREEN, format!("Bağlandı · {} · {:.2} sn", details.server, report.elapsed.as_secs_f64()));
                                    if !details.supports_tools { ui.label("Sunucu araç listeleme yeteneği bildirmedi."); }
                                    else {
                                        ui.collapsing(format!("{} araç", details.tools.len()), |ui| {
                                            if details.tools.is_empty() { ui.label("Sunucunun araç listesi boş."); }
                                            for (name, description) in &details.tools {
                                                ui.strong(name); if !description.is_empty() { ui.label(description); }
                                            }
                                        });
                                    }
                                }
                                Err(error) => { ui.colored_label(Color32::LIGHT_RED, format!("{error} ({:.2} sn)", report.elapsed.as_secs_f64())); }
                            }
                        }
                        ui.separator();
                    });
                }
            });
        if self.open
            && !open
            && let Some(job) = &mut self.job
            && let Some(cancel) = job.cancel.take()
        {
            let _ = cancel.send(());
        }
        self.open = open;
        if let Some(index) = start {
            self.start(index, ctx);
        }
        if self.busy() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Instant;

    #[test]
    fn window_loads_without_running_and_collects_background_results() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        let config = json!({"mcpServers":{"fixture":{
            "command":"python3",
            "args":["-u",concat!(env!("CARGO_MANIFEST_DIR"),"/tests/fixtures/mcp_server.py")]
        }}});
        std::fs::write(&path, config.to_string()).unwrap();
        let mut panel = Panel::default();
        let ctx = egui::Context::default();
        panel.open(&path, dir.path());
        assert_eq!(panel.rows.len(), 1);
        assert!(!panel.busy());
        let output = ctx.run(Default::default(), |ctx| panel.show(ctx));
        assert!(!output.shapes.is_empty());
        panel.start(0, &ctx);
        assert!(panel.busy());
        let deadline = Instant::now() + Duration::from_secs(5);
        while panel.busy() && Instant::now() < deadline {
            let _ = ctx.run(Default::default(), |ctx| panel.show(ctx));
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!panel.busy());
        let details = panel.rows[0]
            .report
            .as_ref()
            .unwrap()
            .outcome
            .as_ref()
            .ok()
            .unwrap();
        assert_eq!(details.tools.len(), 2);
        assert_eq!(details.tools[0].1, dir.path().to_string_lossy());
    }
}
