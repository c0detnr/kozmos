use crate::workspace::{Kind, Workspace};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use std::{
    collections::VecDeque,
    fs,
    time::{Duration, Instant},
};

const BG: Color32 = Color32::from_rgb(9, 13, 23);
const LINE: Color32 = Color32::from_rgb(33, 44, 62);
const WHITE: Color32 = Color32::from_rgb(233, 240, 249);
const MUTED: Color32 = Color32::from_rgb(129, 147, 170);
const THEMES: [Color32; 3] = [
    Color32::from_rgb(90, 235, 198),
    Color32::from_rgb(177, 146, 255),
    Color32::from_rgb(255, 181, 105),
];

#[derive(Clone, Default)]
struct Cpu {
    total: u64,
    idle: u64,
}
fn cpu_rows(raw: &str) -> Vec<Cpu> {
    raw.lines()
        .filter(|l| l.starts_with("cpu"))
        .filter_map(|l| {
            let fields: Vec<u64> = l
                .split_whitespace()
                .skip(1)
                .take(8)
                .map(str::parse)
                .collect::<Result<_, _>>()
                .ok()?;
            if fields.len() < 4 {
                return None;
            }
            Some(Cpu {
                total: fields.iter().sum(),
                idle: fields[3] + fields.get(4).copied().unwrap_or(0),
            })
        })
        .collect()
}
fn cpu_load(old: &Cpu, new: &Cpu) -> f32 {
    let total = new.total.saturating_sub(old.total);
    if total == 0 {
        return 0.0;
    }
    (100.0 * (1.0 - new.idle.saturating_sub(old.idle) as f32 / total as f32)).clamp(0.0, 100.0)
}
fn mem_values(raw: &str) -> (u64, u64) {
    let field = |name: &str| {
        raw.lines()
            .find(|l| l.starts_with(name))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
    };
    let total = field("MemTotal:");
    (total.saturating_sub(field("MemAvailable:")), total)
}
fn network_bytes(raw: &str) -> (u64, u64) {
    raw.lines()
        .filter_map(|line| {
            let (name, values) = line.split_once(':')?;
            if name.trim() == "lo" {
                return None;
            }
            let parts: Vec<_> = values.split_whitespace().collect();
            Some((
                parts.first()?.parse::<u64>().ok()?,
                parts.get(8)?.parse::<u64>().ok()?,
            ))
        })
        .fold((0, 0), |a, b| {
            (a.0.saturating_add(b.0), a.1.saturating_add(b.1))
        })
}
#[derive(Clone)]
struct Process {
    name: String,
    pid: u32,
    memory: u64,
}
fn processes() -> Vec<Process> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse::<u32>().ok())
            else {
                continue;
            };
            if let Ok(status) = fs::read_to_string(entry.path().join("status")) {
                let name = status
                    .lines()
                    .find_map(|l| l.strip_prefix("Name:\t"))
                    .unwrap_or("?")
                    .to_owned();
                let memory = status
                    .lines()
                    .find_map(|l| l.strip_prefix("VmRSS:"))
                    .and_then(|l| l.split_whitespace().next())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                if memory > 0 {
                    out.push(Process { name, pid, memory });
                }
            }
        }
    }
    out.sort_by_key(|p| std::cmp::Reverse(p.memory));
    out
}
struct Telemetry {
    previous: Vec<Cpu>,
    cores: Vec<f32>,
    cpu: f32,
    used: u64,
    total: u64,
    history: VecDeque<(f32, f32)>,
    net: (u64, u64),
    down: f32,
    up: f32,
    last: Instant,
    processes: Vec<Process>,
    host: String,
    kernel: String,
    disk: Option<(f64, f64)>,
    uptime: u64,
    error: bool,
}
impl Telemetry {
    fn new() -> Self {
        let disk = std::process::Command::new("df")
            .args(["-Pk", "/"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| {
                let s = String::from_utf8(o.stdout).ok()?;
                let fields: Vec<_> = s.lines().nth(1)?.split_whitespace().collect();
                Some((
                    fields.get(2)?.parse::<f64>().ok()? / 1048576.0,
                    fields.get(1)?.parse::<f64>().ok()? / 1048576.0,
                ))
            });
        Self {
            previous: cpu_rows(&fs::read_to_string("/proc/stat").unwrap_or_default()),
            cores: vec![],
            cpu: 0.0,
            used: 0,
            total: 0,
            history: VecDeque::new(),
            net: network_bytes(&fs::read_to_string("/proc/net/dev").unwrap_or_default()),
            down: 0.0,
            up: 0.0,
            last: Instant::now(),
            processes: processes(),
            host: fs::read_to_string("/proc/sys/kernel/hostname")
                .unwrap_or_default()
                .trim()
                .into(),
            kernel: fs::read_to_string("/proc/sys/kernel/osrelease")
                .unwrap_or_default()
                .trim()
                .into(),
            disk,
            uptime: 0,
            error: false,
        }
    }
    fn sample(&mut self) {
        let elapsed = self.last.elapsed().as_secs_f32();
        if elapsed < 1.0 {
            return;
        }
        self.last = Instant::now();
        let stat = fs::read_to_string("/proc/stat");
        let mem = fs::read_to_string("/proc/meminfo");
        self.error = stat.is_err() || mem.is_err();
        let current = cpu_rows(&stat.unwrap_or_default());
        let loads: Vec<_> = current
            .iter()
            .zip(&self.previous)
            .map(|(n, o)| cpu_load(o, n))
            .collect();
        self.cpu = loads.first().copied().unwrap_or(0.0);
        self.cores = loads.into_iter().skip(1).collect();
        self.previous = current;
        (self.used, self.total) = mem_values(&mem.unwrap_or_default());
        let net = network_bytes(&fs::read_to_string("/proc/net/dev").unwrap_or_default());
        self.down = net.0.saturating_sub(self.net.0) as f32 / elapsed / 1024.0;
        self.up = net.1.saturating_sub(self.net.1) as f32 / elapsed / 1024.0;
        self.net = net;
        self.history.push_back((self.cpu, self.memory_pct()));
        if self.history.len() > 90 {
            self.history.pop_front();
        }
        self.processes = processes();
        self.uptime = fs::read_to_string("/proc/uptime")
            .ok()
            .and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok())
            .unwrap_or(0.0) as u64;
    }
    fn memory_pct(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.used as f32 / self.total as f32 * 100.0
        }
    }
}

#[derive(Default)]
struct ProjectSummary {
    path: Option<std::path::PathBuf>,
    counts: [usize; 3],
    enabled_skills: usize,
    global_count: usize,
    providers: Vec<String>,
    warnings: Vec<String>,
}
impl ProjectSummary {
    fn from_workspace(ws: &Workspace) -> Self {
        let mut summary = Self {
            path: ws.settings.active.clone(),
            warnings: ws.warnings.clone(),
            ..Self::default()
        };
        for resource in &ws.resources {
            if resource.global {
                summary.global_count += 1;
                continue;
            }
            let index = match resource.kind {
                Kind::Skill => 0,
                Kind::Mcp => 1,
                Kind::Instructions => 2,
            };
            summary.counts[index] += 1;
            if resource.kind == Kind::Skill && resource.enabled {
                summary.enabled_skills += 1;
            }
            if !summary.providers.contains(&resource.provider) {
                summary.providers.push(resource.provider.clone());
            }
        }
        summary.providers.sort();
        summary
    }
}
pub(crate) struct Kozmos {
    project: ProjectSummary,
    pub(crate) open_resource: Option<Kind>,
    pub(crate) open_projects: bool,
    pub(crate) refresh_requested: bool,
    data: Telemetry,
    animation_time: f32,
    paused: bool,
    focus_remaining: f32,
    focus_running: bool,
    last_frame: Instant,
    status: String,
    space: egui::TextureHandle,
    editing: bool,
    draft: String,
}

// Cache the planetary texture; animate its camera and lightweight foreground geometry.
fn space_image() -> egui::ColorImage {
    let (w, h) = (1440, 600);
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / w as f32;
            let v = y as f32 / h as f32;
            let dx = (u - 0.79) * 2.4;
            let dy = (v - 1.72) * 1.0;
            let r = (dx * dx + dy * dy).sqrt();
            let edge = r - 1.22;
            let light = (0.72 + dx * 0.4 - dy * 0.18).clamp(0.0, 1.0);
            let halo = (-edge.abs() * 65.0).exp() * light;
            let haze = (-edge.abs() * 14.0).exp() * light;
            let grain = (((x as u32 * 1973) ^ (y as u32 * 9277)).wrapping_mul(26699) % 1000) as f32
                / 1000.0;
            let mut rgb = [3.0 + grain * 1.5, 7.0 + grain * 1.5, 15.0 + grain * 2.0];
            if edge < 0.0 {
                let texture = ((dx * 24.0 + (dy * 39.0).sin() * 2.0).sin()
                    * (dy * 64.0 + (dx * 31.0).cos()).cos()
                    * 0.5
                    + 0.5)
                    .powf(3.0);
                let depth = (1.0 + edge * 2.5).max(0.0);
                rgb = [
                    4.0 + texture * 15.0 * depth,
                    11.0 + texture * 27.0 * depth,
                    21.0 + texture * 36.0 * depth,
                ];
            }
            let fade = ((u - 0.15) * 1.5).clamp(0.0, 1.0);
            rgb[0] += (halo * 115.0 + haze * 11.0) * fade;
            rgb[1] += (halo * 180.0 + haze * 35.0) * fade;
            rgb[2] += (halo * 215.0 + haze * 70.0) * fade;
            pixels.push(Color32::from_rgb(
                rgb[0].min(255.0) as u8,
                rgb[1].min(255.0) as u8,
                rgb[2].min(255.0) as u8,
            ));
        }
    }
    egui::ColorImage::new([w, h], pixels)
}

impl Kozmos {
    pub(crate) fn new(ctx: &egui::Context) -> Self {
        Self {
            project: ProjectSummary::default(),
            open_resource: None,
            open_projects: false,
            refresh_requested: false,
            data: Telemetry::new(),
            animation_time: 0.0,
            paused: false,
            focus_remaining: 1500.0,
            focus_running: false,
            last_frame: Instant::now(),
            status: String::new(),
            editing: false,
            draft: String::new(),
            space: ctx.load_texture(
                "planetary-horizon",
                space_image(),
                egui::TextureOptions::LINEAR,
            ),
        }
    }
    pub(crate) fn sync_project(&mut self, ws: &Workspace) {
        if self.project.path != ws.settings.active {
            self.editing = false;
            self.draft.clear();
            self.status.clear();
        }
        self.project = ProjectSummary::from_workspace(ws);
    }
    pub(crate) fn report_save(&mut self, result: Result<(), String>) {
        self.status = match result {
            Ok(()) => "Proje adı kaydedildi.".into(),
            Err(e) => format!("Proje adı kaydedilemedi: {e}"),
        };
    }
    fn export(&mut self) {
        let base = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let path = base.join(format!("kozmos-{stamp}.csv"));
        let mut csv = String::from("sample,cpu_percent,memory_percent\n");
        for (i, (cpu, mem)) in self.data.history.iter().enumerate() {
            csv.push_str(&format!("{i},{cpu:.2},{mem:.2}\n"));
        }
        self.status = match fs::write(&path, csv) {
            Ok(()) => format!("Kaydedildi: {}", path.display()),
            Err(e) => format!("Kayıt hatası: {e}"),
        };
    }
    pub(crate) fn show(&mut self, ctx: &egui::Context, name: &mut String) -> bool {
        let dt = self.last_frame.elapsed().as_secs_f32();
        self.last_frame = Instant::now();
        self.data.sample();
        if !self.paused {
            self.animation_time += dt.min(0.1);
        }
        if self.focus_running {
            self.focus_remaining = (self.focus_remaining - dt).max(0.0);
            if self.focus_remaining == 0.0 {
                self.focus_running = false;
                self.status = "Odak süresi tamamlandı.".into();
            }
        }
        if !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            self.paused = !self.paused;
        }
        let mut changed = false;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(BG))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("system-home")
                    .show(ui, |ui| {
                        changed = self.banner(ui, name);
                        egui::Frame::new().inner_margin(28).show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            self.project_panel(ui);
                            ui.add_space(32.0);
                            ui.separator();
                            ui.add_space(24.0);
                            ui.label(
                                egui::RichText::new("Bilgisayar ve donanım")
                                    .size(20.0)
                                    .color(WHITE),
                            );
                            ui.add_space(8.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.label(egui::RichText::new("SİSTEM").size(11.0).color(MUTED));
                                ui.label(
                                    egui::RichText::new(if self.data.error {
                                        "● Veri okunamadı"
                                    } else {
                                        "● Canlı"
                                    })
                                    .size(11.0)
                                    .color(
                                        if self.data.error {
                                            THEMES[2]
                                        } else {
                                            THEMES[0]
                                        },
                                    ),
                                );
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{}  /  Linux {}  /  {} sa {} dk",
                                        self.data.host,
                                        self.data.kernel,
                                        self.data.uptime / 3600,
                                        self.data.uptime % 3600 / 60
                                    ))
                                    .size(11.0)
                                    .color(MUTED),
                                );
                            });
                            ui.add_space(20.0);
                            let metrics = [
                                (
                                    "İŞLEMCİ",
                                    format!("{:.1}%", self.data.cpu),
                                    format!("{} çekirdek", self.data.cores.len()),
                                    self.data.cpu,
                                ),
                                (
                                    "BELLEK",
                                    format!("{:.1} GB", self.data.used as f64 / 1048576.0),
                                    format!("{:.1} GB toplam", self.data.total as f64 / 1048576.0),
                                    self.data.memory_pct(),
                                ),
                                (
                                    "DEPOLAMA",
                                    self.data
                                        .disk
                                        .map(|d| format!("{:.0} GB", d.0))
                                        .unwrap_or("—".into()),
                                    self.data
                                        .disk
                                        .map(|d| format!("{:.0} GB toplam", d.1))
                                        .unwrap_or("Okunamadı".into()),
                                    self.data
                                        .disk
                                        .map(|d| (d.0 / d.1.max(1.0) * 100.0) as f32)
                                        .unwrap_or(0.0),
                                ),
                                (
                                    "AĞ",
                                    format!("{:.0} KB/s", self.data.down),
                                    format!("↑ {:.0} KB/s", self.data.up),
                                    0.0,
                                ),
                            ];
                            let columns = if ui.available_width() < 650.0 { 2 } else { 4 };
                            for row in metrics.chunks(columns) {
                                ui.columns(columns, |cols| {
                                    for (col, (label, value, detail, pct)) in
                                        cols.iter_mut().zip(row)
                                    {
                                        col.label(
                                            egui::RichText::new(*label).size(10.0).color(MUTED),
                                        );
                                        col.label(
                                            egui::RichText::new(value).size(30.0).color(WHITE),
                                        );
                                        col.label(
                                            egui::RichText::new(detail).size(11.0).color(MUTED),
                                        );
                                        col.add_space(10.0);
                                        let (r, _) = col.allocate_exact_size(
                                            Vec2::new(col.available_width(), 2.0),
                                            Sense::hover(),
                                        );
                                        col.painter().rect_filled(r, 0.0, LINE);
                                        if *pct > 0.0 {
                                            col.painter().rect_filled(
                                                Rect::from_min_size(
                                                    r.min,
                                                    Vec2::new(r.width() * pct / 100.0, 2.0),
                                                ),
                                                0.0,
                                                THEMES[0],
                                            );
                                        }
                                    }
                                });
                                ui.add_space(20.0);
                            }
                            ui.add_space(12.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.label(egui::RichText::new("Kaynak kullanımı").size(17.0));
                                ui.label(egui::RichText::new("CPU").size(11.0).color(THEMES[0]));
                                ui.label(egui::RichText::new("RAM").size(11.0).color(THEMES[1]));
                                ui.label(
                                    egui::RichText::new("90 örnek · 1 sn")
                                        .size(11.0)
                                        .color(MUTED),
                                );
                            });
                            self.chart(ui);
                            ui.add_space(24.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.label(egui::RichText::new("Süreçler").size(17.0));
                                ui.label(
                                    egui::RichText::new("Bellek kullanımına göre")
                                        .size(11.0)
                                        .color(MUTED),
                                );
                            });
                            egui::Grid::new("system-processes")
                                .num_columns(3)
                                .spacing([24.0, 12.0])
                                .min_col_width((ui.available_width() - 48.0) / 3.0)
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new("SÜREÇ").color(MUTED).size(10.0));
                                    ui.label(egui::RichText::new("PID").color(MUTED).size(10.0));
                                    ui.label(egui::RichText::new("BELLEK").color(MUTED).size(10.0));
                                    ui.end_row();
                                    for p in self.data.processes.iter().take(6) {
                                        ui.add(egui::Label::new(&p.name).truncate());
                                        ui.monospace(p.pid.to_string());
                                        ui.monospace(format!("{:.0} MB", p.memory as f64 / 1024.0));
                                        ui.end_row();
                                    }
                                });
                            ui.add_space(24.0);
                            ui.separator();
                            ui.add_space(12.0);
                            if ui.button("CSV dışa aktar").clicked() {
                                self.export();
                            }
                            if !self.status.is_empty() {
                                ui.add_space(10.0);
                                ui.label(egui::RichText::new(&self.status).size(11.0).color(MUTED));
                            }
                        });
                    });
            });
        ctx.request_repaint_after(Duration::from_millis(if self.paused { 250 } else { 33 }));
        changed
    }
    fn project_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Aktif proje").size(20.0).color(WHITE));
            if ui.small_button("Proje değiştir").clicked() {
                self.open_projects = true;
            }
            if ui.small_button("Yenile").clicked() {
                self.refresh_requested = true;
            }
        });
        if let Some(path) = &self.project.path {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(path.display().to_string())
                        .monospace()
                        .size(12.0)
                        .color(MUTED),
                )
                .wrap(),
            );
        } else {
            ui.label(
                egui::RichText::new("Aktif proje seçilmedi. Projelerden bir klasör seç.")
                    .color(MUTED),
            );
        }
        ui.add_space(20.0);
        let items = [
            (
                Kind::Skill,
                "SKILLS",
                format!(
                    "{} etkin · {} devre dışı",
                    self.project.enabled_skills,
                    self.project.counts[0] - self.project.enabled_skills
                ),
            ),
            (
                Kind::Mcp,
                "MCP YAPILANDIRMALARI",
                "Projedeki yapılandırma dosyaları".into(),
            ),
            (
                Kind::Instructions,
                "AGENT TALİMATLARI",
                "Projedeki talimat dosyaları".into(),
            ),
        ];
        let columns = if ui.available_width() < 600.0 { 1 } else { 3 };
        for (row_index, row) in items.chunks(columns).enumerate() {
            ui.columns(columns, |cols| {
                for (index, (col, (kind, title, detail))) in cols.iter_mut().zip(row).enumerate() {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(13, 21, 32))
                        .inner_margin(18)
                        .corner_radius(8)
                        .show(col, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.label(egui::RichText::new(*title).size(10.0).color(MUTED));
                            ui.label(
                                egui::RichText::new(
                                    self.project.counts[row_index * columns + index].to_string(),
                                )
                                .size(34.0)
                                .color(WHITE),
                            );
                            ui.label(egui::RichText::new(detail).size(11.0).color(MUTED));
                            ui.add_space(8.0);
                            if ui
                                .add_enabled(
                                    self.project.path.is_some(),
                                    egui::Button::new("Yönet →").frame(false),
                                )
                                .clicked()
                            {
                                self.open_resource = Some(*kind);
                            }
                        });
                }
            });
            ui.add_space(10.0);
        }
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new("PROJE KAYNAKLARI")
                    .size(10.0)
                    .color(MUTED),
            );
            ui.label(
                egui::RichText::new(if self.project.providers.is_empty() {
                    "Henüz kaynak yok".into()
                } else {
                    self.project.providers.join(" / ")
                })
                .size(12.0),
            );
        });
        ui.label(
            egui::RichText::new(format!(
                "Global envanter: {} kaynak · Yukarıdaki sayılara dahil değil",
                self.project.global_count
            ))
            .size(11.0)
            .color(MUTED),
        );
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("ODAK").size(10.0).color(MUTED));
            let seconds = self.focus_remaining.ceil() as u32;
            ui.monospace(format!("{:02}:{:02}", seconds / 60, seconds % 60));
            if ui
                .button(if self.focus_running {
                    "Duraklat"
                } else {
                    "Başlat"
                })
                .clicked()
            {
                if self.focus_remaining <= 0.0 {
                    self.focus_remaining = 1500.0;
                }
                self.focus_running = !self.focus_running;
            }
            if ui.button("Sıfırla").clicked() {
                self.focus_running = false;
                self.focus_remaining = 1500.0;
            }
        });

        for warning in &self.project.warnings {
            ui.label(egui::RichText::new(warning).size(11.0).color(THEMES[2]));
        }
    }
    fn paint_space(&self, painter: &egui::Painter, r: Rect) {
        let t = self.animation_time;
        // Overscan keeps the frame filled while the camera drifts over the horizon.
        let zoom = 0.91 + 0.015 * (t * 0.12).sin();
        let offset = Vec2::new(
            0.045 + 0.024 * (t * 0.09).sin(),
            0.045 + 0.018 * (t * 0.13).cos(),
        );
        painter.image(
            self.space.id(),
            r,
            Rect::from_min_size(Pos2::new(offset.x, offset.y), Vec2::splat(zoom)),
            Color32::WHITE,
        );
        for i in 0..220 {
            let seed = i as f32;
            let depth = (i % 4) as f32 / 3.0;
            let x = (((i * 137 + 29) % 997) as f32 / 997.0 + t * (0.002 + depth * 0.003)).fract();
            let y = (((i * 233 + 19) % 991) as f32 / 991.0 + 0.008 * (t * 0.18 + seed).sin())
                .rem_euclid(1.0);
            // Match the texture's sphere silhouette so stars remain behind the planet.
            let u = offset.x + x * zoom;
            let v = offset.y + y * zoom;
            let radius = (((u - 0.79) * 2.4).powi(2) + (v - 1.72).powi(2)).sqrt();
            let visibility = ((radius - 1.24) * 18.0).clamp(0.0, 1.0);
            let alpha =
                ((115.0 + 65.0 * (seed + t * (0.65 + depth * 0.25)).sin()) * visibility) as u8;
            let p = r.min + Vec2::new(x * r.width(), y * r.height());
            if i % 17 == 0 {
                painter.circle_filled(
                    p,
                    3.0,
                    Color32::from_rgba_unmultiplied(145, 193, 255, alpha / 8),
                );
            }
            painter.circle_filled(
                p,
                0.5 + depth * 0.8,
                Color32::from_rgba_unmultiplied(193, 215, 244, alpha),
            );
        }
        // A short, fading meteor pass every twelve seconds, confined to the upper sky.
        let phase = (t + 2.0).rem_euclid(12.0);
        if phase < 1.6 {
            let progress = phase / 1.6;
            let opacity = (progress * std::f32::consts::PI).sin();
            let head = r.min
                + Vec2::new(
                    r.width() * (0.38 + progress * 0.45),
                    r.height() * (0.09 + progress * 0.22),
                );
            let tail = Vec2::new(r.width() * 0.10, r.height() * 0.049);
            for segment in 0..18 {
                let fraction = segment as f32 / 18.0;
                painter.line_segment(
                    [
                        head - tail * fraction,
                        head - tail * ((segment + 1) as f32 / 18.0),
                    ],
                    Stroke::new(
                        1.2_f32,
                        Color32::from_rgba_unmultiplied(
                            179,
                            216,
                            255,
                            (opacity * (1.0 - fraction).powi(2) * 180.0) as u8,
                        ),
                    ),
                );
            }
            painter.circle_filled(
                head,
                1.5,
                Color32::from_white_alpha((opacity * 220.0) as u8),
            );
        }
    }
    fn banner(&mut self, ui: &mut egui::Ui, name: &mut String) -> bool {
        let width = ui.available_width();
        let height = (width * 0.42).clamp(300.0, 480.0);
        let (r, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
        let painter = ui.painter().with_clip_rect(r);
        self.paint_space(&painter, r);
        painter.text(
            r.min + Vec2::new(32.0, 26.0),
            Align2::LEFT_TOP,
            "K O Z M O S   /   P R O J E",
            FontId::monospace(10.0),
            MUTED,
        );
        let button_rect = Rect::from_min_size(
            Pos2::new(r.right() - 118.0, r.top() + 20.0),
            Vec2::new(94.0, 28.0),
        );
        if ui
            .put(
                button_rect,
                egui::Button::new(if self.paused {
                    "▶ Hareket"
                } else {
                    "Ⅱ Duraklat"
                })
                .frame(false),
            )
            .clicked()
        {
            self.paused = !self.paused;
        }
        let title_rect = Rect::from_min_size(
            r.min + Vec2::new(32.0, height * 0.44),
            Vec2::new((width - 64.0).max(80.0), 66.0),
        );
        let mut changed = false;
        if self.editing {
            let response = ui.put(
                title_rect,
                egui::TextEdit::singleline(&mut self.draft)
                    .font(FontId::proportional(44.0))
                    .text_color(WHITE)
                    .char_limit(64)
                    .hint_text("Project")
                    .frame(false),
            );
            if !response.has_focus() && !response.lost_focus() {
                response.request_focus();
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.editing = false;
            } else if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let value = self.draft.trim();
                let value = if value.is_empty() { "Project" } else { value };
                if name != value {
                    *name = value.into();
                    changed = true;
                }
                self.editing = false;
            }
        } else {
            let label = if name.is_empty() {
                "Project"
            } else {
                name.as_str()
            };
            let response = ui.put(
                title_rect,
                egui::Button::new(egui::RichText::new(label).size(44.0).color(WHITE))
                    .frame(false)
                    .wrap()
                    .min_size(Vec2::ZERO),
            );
            response.clone().on_hover_text("Proje adını düzenle");
            if response.clicked() {
                self.draft = label.into();
                self.editing = true;
            }
        }
        painter.text(
            r.min + Vec2::new(32.0, height - 35.0),
            Align2::LEFT_TOP,
            if self.editing {
                "ENTER  Kaydet     ESC  İptal"
            } else {
                "PROJE ADI  /  DÜZENLEMEK İÇİN TIKLA"
            },
            FontId::monospace(9.0),
            MUTED,
        );
        painter.line_segment(
            [r.left_bottom(), r.right_bottom()],
            Stroke::new(1.0_f32, LINE),
        );
        changed
    }
    fn chart(&self, ui: &mut egui::Ui) {
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 135.0), Sense::hover());
        let painter = ui.painter().with_clip_rect(r);
        for n in 0..=3 {
            let y = r.top() + 12.0 + n as f32 * 38.0;
            painter.line_segment(
                [Pos2::new(r.left(), y), Pos2::new(r.right(), y)],
                Stroke::new(1.0_f32, LINE.gamma_multiply(0.5)),
            );
        }
        for (series, color) in [(0, THEMES[0]), (1, THEMES[1])] {
            let points: Vec<_> = self
                .data
                .history
                .iter()
                .enumerate()
                .map(|(i, values)| {
                    Pos2::new(
                        r.right() - (self.data.history.len() - 1 - i) as f32 * r.width() / 89.0,
                        r.bottom() - 9.0 - if series == 0 { values.0 } else { values.1 } * 1.14,
                    )
                })
                .collect();
            if points.len() > 1 {
                painter.add(egui::Shape::line(points, Stroke::new(1.5_f32, color)));
            }
        }
        if self.data.history.len() < 2 {
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                "Örnekler bekleniyor",
                FontId::proportional(12.0),
                MUTED,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cpu_excludes_guest_and_counts_iowait_as_idle() {
        let rows = cpu_rows("cpu 100 0 50 700 50 0 0 0 100 0\ncpu0 50 0 25 350 25 0 0 0\nintr 3");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].total, 900);
        assert_eq!(rows[0].idle, 750);
        assert_eq!(
            cpu_load(
                &Cpu {
                    total: 800,
                    idle: 700
                },
                &rows[0]
            ),
            50.0
        );
        assert_eq!(cpu_load(&rows[0], &rows[0]), 0.0);
    }
    #[test]
    fn memory_uses_available_including_reclaimable_cache() {
        assert_eq!(
            mem_values("MemTotal: 1000 kB\nMemFree: 100 kB\nMemAvailable: 600 kB\n"),
            (400, 1000)
        );
        assert_eq!(mem_values(""), (0, 0));
    }
    #[test]
    fn network_sums_interfaces_but_excludes_loopback() {
        assert_eq!(
            network_bytes(
                "lo: 999 0 0 0 0 0 0 0 999\nenp1s0: 1024 0 0 0 0 0 0 0 2048\nwlan0: 50 0 0 0 0 0 0 0 70"
            ),
            (1074, 2118)
        );
    }
}

#[cfg(test)]
mod interaction_tests {
    use super::*;
    fn render(
        app: &mut Kozmos,
        ctx: &egui::Context,
        name: &mut String,
        events: Vec<egui::Event>,
        width: f32,
    ) -> bool {
        let mut changed = false;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 900.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                changed = app.show(ctx, name);
            },
        );
        assert!(!output.shapes.is_empty());
        changed
    }
    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }
    }
    #[test]
    fn project_summary_excludes_global_resources_and_resets_editor_on_switch() {
        let temp = tempfile::tempdir().unwrap();
        let mut ws = Workspace::new(temp.path().join("home"), temp.path().join("state.json"));
        ws.settings.active = Some(temp.path().join("project-a"));
        for (kind, global, enabled) in [
            (Kind::Skill, false, true),
            (Kind::Skill, false, false),
            (Kind::Mcp, false, true),
            (Kind::Instructions, false, true),
            (Kind::Skill, true, true),
        ] {
            ws.resources.push(crate::workspace::Resource {
                name: "Test".into(),
                description: String::new(),
                path: temp.path().join("resource"),
                kind,
                global,
                provider: "Codex".into(),
                enabled,
            });
        }
        let ctx = egui::Context::default();
        let mut app = Kozmos::new(&ctx);
        app.sync_project(&ws);
        assert_eq!(app.project.counts, [2, 1, 1]);
        assert_eq!(app.project.enabled_skills, 1);
        assert_eq!(app.project.global_count, 1);
        assert_eq!(app.project.providers, ["Codex"]);
        app.editing = true;
        app.draft = "Project A".into();
        ws.settings.active = Some(temp.path().join("project-b"));
        app.sync_project(&ws);
        assert!(!app.editing);
        assert!(app.draft.is_empty());
    }
    #[test]
    fn title_commit_cancel_and_typing_do_not_pause_animation() {
        let ctx = egui::Context::default();
        let mut app = Kozmos::new(&ctx);
        let mut name = "Project".to_owned();
        render(&mut app, &ctx, &mut name, vec![], 1000.0);
        for pressed in [true, false] {
            render(
                &mut app,
                &ctx,
                &mut name,
                vec![
                    egui::Event::PointerMoved(Pos2::new(500.0, 210.0)),
                    egui::Event::PointerButton {
                        pos: Pos2::new(500.0, 210.0),
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
                1000.0,
            );
        }
        assert!(app.editing);
        render(&mut app, &ctx, &mut name, vec![], 1000.0);
        render(
            &mut app,
            &ctx,
            &mut name,
            vec![key(egui::Key::Space)],
            1000.0,
        );
        assert!(!app.paused);
        app.draft = "  Mission  ".into();
        assert!(render(
            &mut app,
            &ctx,
            &mut name,
            vec![key(egui::Key::Enter)],
            1000.0
        ));
        assert_eq!(name, "Mission");
        app.editing = true;
        app.draft = "Discard".into();
        render(&mut app, &ctx, &mut name, vec![], 1000.0);
        assert!(!render(
            &mut app,
            &ctx,
            &mut name,
            vec![key(egui::Key::Escape)],
            1000.0
        ));
        assert_eq!(name, "Mission");
        assert!(!app.editing);
        render(&mut app, &ctx, &mut name, vec![], 450.0);
    }
}
