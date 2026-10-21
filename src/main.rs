mod brand;
mod mcp;
mod mcp_ui;
mod telemetry;
mod workspace;

use eframe::egui::{self, Color32, RichText, Vec2};
use std::{path::PathBuf, time::Duration};
use workspace::{Kind, Resource, Workspace};

const BG: Color32 = Color32::from_rgb(12, 16, 24);
const PANEL: Color32 = Color32::from_rgb(19, 25, 36);
const LINE: Color32 = Color32::from_rgb(39, 49, 65);
const TEXT: Color32 = Color32::from_rgb(228, 235, 245);
const MUTED: Color32 = Color32::from_rgb(137, 153, 176);
const ACCENT: Color32 = Color32::from_rgb(117, 234, 197);

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Overview,
    Skills,
    Mcp,
    Instructions,
    Projects,
    System,
}
impl Page {
    fn kind(self) -> Option<Kind> {
        match self {
            Self::Skills => Some(Kind::Skill),
            Self::Mcp => Some(Kind::Mcp),
            Self::Instructions => Some(Kind::Instructions),
            _ => None,
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "Kontrol merkezi",
            Self::Skills => "Skill kütüphanesi",
            Self::Mcp => "MCP bağlantıları",
            Self::Instructions => "Agent talimatları",
            Self::Projects => "Çalışma alanları",
            Self::System => "Proje dashboardı",
        }
    }
}
struct Editor {
    resource: Resource,
    original: Option<String>,
    content: String,
    discard: bool,
}
impl Editor {
    fn dirty(&self) -> bool {
        self.original.as_deref() != Some(self.content.as_str())
    }
}
struct Create {
    kind: Kind,
    global: bool,
    name: String,
    provider: String,
}
struct AgentOs {
    ws: Workspace,
    page: Page,
    scope: u8,
    search: String,
    project_input: String,
    source_input: String,
    source_kind: Kind,
    editor: Option<Editor>,
    create: Option<Create>,
    message: String,
    failed: bool,
    telemetry: telemetry::Kozmos,
    mcp_tests: mcp_ui::Panel,
}
impl AgentOs {
    fn new(ctx: &egui::Context) -> Self {
        let mut style = (*ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = BG;
        style.visuals.window_fill = PANEL;
        style.visuals.extreme_bg_color = Color32::from_rgb(9, 13, 20);
        style.visuals.widgets.inactive.bg_fill = PANEL;
        style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, LINE);
        style.visuals.selection.bg_fill = Color32::from_rgb(34, 78, 70);
        style.visuals.selection.stroke = egui::Stroke::new(1.0_f32, ACCENT);
        style.visuals.override_text_color = Some(TEXT);
        style.spacing.item_spacing = Vec2::new(10.0, 10.0);
        style.spacing.button_padding = Vec2::new(12.0, 8.0);
        ctx.set_style(style);
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let mut ws = Workspace::new(home, config.join("kozmos/workspaces.json"));
        let startup_project = std::env::args_os().nth(1).map(PathBuf::from).or_else(|| {
            ws.settings
                .projects
                .is_empty()
                .then(|| std::env::current_dir().ok())
                .flatten()
        });
        let startup_error =
            startup_project.and_then(|path| ws.add_project(&path.to_string_lossy()).err());
        Self {
            ws,
            page: Page::Overview,
            scope: 0,
            search: String::new(),
            project_input: String::new(),
            source_input: String::new(),
            source_kind: Kind::Skill,
            editor: None,
            create: None,
            failed: startup_error.is_some(),
            message: startup_error
                .unwrap_or_else(|| "Yerel envanter hazır. Kaynaklar cihazından okunuyor.".into()),
            telemetry: telemetry::Kozmos::new(ctx),
            mcp_tests: mcp_ui::Panel::default(),
        }
    }
    fn result<T>(&mut self, result: Result<T, String>, success: &str) {
        match result {
            Ok(_) => {
                self.message = success.into();
                self.failed = false;
                self.ws.refresh();
            }
            Err(e) => {
                self.message = e;
                self.failed = true;
            }
        }
    }
    fn open(&mut self, resource: Resource) {
        if self.editor.as_ref().is_some_and(Editor::dirty) {
            self.result::<()>(
                Err("Önce açık dosyayı kaydedin veya düzenleyicide değişiklikleri bırakın.".into()),
                "",
            );
            return;
        }
        match workspace::read_document(&resource.path) {
            Ok(content) => {
                self.editor = Some(Editor {
                    resource,
                    original: Some(content.clone()),
                    content,
                    discard: false,
                })
            }
            Err(e) => self.result::<()>(Err(e), ""),
        }
    }
    fn new_resource(&mut self, kind: Kind) {
        self.create = Some(Create {
            kind,
            global: self.scope != 2,
            name: String::new(),
            provider: "Claude".into(),
        });
    }
    fn activate_project(&mut self, path: &str) -> bool {
        if self.editor.as_ref().is_some_and(Editor::dirty) {
            self.result::<()>(
                Err("Önce açık dosyayı kaydedin veya değişiklikleri bırakın.".into()),
                "",
            );
            return false;
        }
        let result = self.ws.add_project(path);
        let activated = result.is_ok();
        if activated {
            self.editor = None;
            self.create = None;
        }
        self.result(result, "Aktif proje değiştirildi.");
        activated
    }

    fn project_selector(&mut self, ui: &mut egui::Ui) {
        let active = self.ws.settings.active.clone();
        let name = active
            .as_ref()
            .map(|path| {
                path.file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy()
                    .into_owned()
            })
            .unwrap_or_else(|| "Proje seç".into());
        let response = ui.add_sized(
            [ui.available_width(), 36.0],
            egui::Button::new(RichText::new(&name).size(12.0))
                .truncate()
                .right_text(RichText::new("⌄").color(MUTED))
                .fill(BG)
                .stroke(egui::Stroke::new(1.0_f32, LINE))
                .corner_radius(7),
        );
        let response = response.on_hover_text(
            active
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "Proje seç".into()),
        );
        egui::Popup::menu(&response).width(300.0).show(|ui| {
            ui.label(RichText::new("Son projeler").size(11.0).color(MUTED));
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    if self.ws.settings.projects.is_empty() {
                        ui.label(RichText::new("Henüz proje açılmadı.").color(MUTED));
                    }
                    for path in self.ws.settings.projects.clone() {
                        let selected = active.as_ref() == Some(&path);
                        let name = path
                            .file_name()
                            .unwrap_or(path.as_os_str())
                            .to_string_lossy();
                        let available = path.is_dir();
                        let label = format!(
                            "{}{}\n{}",
                            if selected { "✓  " } else { "" },
                            name,
                            path.display()
                        );
                        let row = ui.add_enabled(
                            available,
                            egui::Button::new(RichText::new(label).size(12.0))
                                .selected(selected)
                                .min_size(Vec2::new(ui.available_width(), 48.0))
                                .truncate(),
                        );
                        let row = row.on_hover_text(if available {
                            path.display().to_string()
                        } else {
                            format!("Klasör bulunamadı: {}", path.display())
                        });
                        if row.clicked() {
                            if !selected {
                                self.activate_project(&path.to_string_lossy());
                            }
                            ui.close();
                        }
                    }
                });
            ui.separator();
            if ui.button("+ Proje aç…").clicked() {
                self.page = Page::Projects;
                ui.close();
            }
        });
    }

    fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("navigation")
            .exact_width(218.0)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(20))
            .show(ctx, |ui| {
                ui.add_space(8.0);
                brand::sidebar_logo(ui);
                ui.add_space(14.0);
                self.project_selector(ui);
                ui.add_space(24.0);
                ui.label(RichText::new("WORKSPACE").size(10.0).color(MUTED));
                for (page, label) in [
                    (Page::Overview, "01   Kontrol merkezi"),
                    (Page::Skills, "02   Skills"),
                    (Page::Mcp, "03   MCP sunucuları"),
                    (Page::Instructions, "04   Agent talimatları"),
                    (Page::Projects, "05   Projeler"),
                ] {
                    if ui
                        .add_sized(
                            [178.0, 40.0],
                            egui::Button::new(label).selected(self.page == page),
                        )
                        .clicked()
                    {
                        self.page = page;
                    }
                }
                ui.add_space(22.0);
                ui.separator();
                ui.add_space(12.0);
                if ui
                    .add_sized(
                        [178.0, 38.0],
                        egui::Button::new("06   Proje dashboardı")
                            .selected(self.page == Page::System),
                    )
                    .clicked()
                {
                    self.page = Page::System;
                }
            });
    }
    fn overview(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        egui::Frame::new().fill(PANEL).corner_radius(16).inner_margin(24).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new("Çalışma alanı").size(29.0).strong());
            ui.label(RichText::new("Skill paketlerini, MCP bağlantılarını ve agent talimatlarını global veya proje kapsamında yönet.").color(MUTED));
            ui.add_space(12.0);
            if ui.button("Proje yönetimine git  →").clicked() { self.page = Page::Projects; }
        });
        ui.add_space(18.0);
        ui.columns(3, |cols| {
            for (i, kind) in [Kind::Skill, Kind::Mcp, Kind::Instructions]
                .iter()
                .enumerate()
            {
                let count = self.ws.resources.iter().filter(|r| r.kind == *kind).count();
                egui::Frame::new()
                    .fill(PANEL)
                    .corner_radius(12)
                    .inner_margin(20)
                    .show(&mut cols[i], |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(
                            RichText::new(count.to_string())
                                .size(34.0)
                                .color(ACCENT)
                                .strong(),
                        );
                        ui.label(if *kind == Kind::Mcp {
                            "MCP yapılandırma dosyası"
                        } else {
                            kind.label()
                        });
                        if ui.small_button("Yönet →").clicked() {
                            self.page = match kind {
                                Kind::Skill => Page::Skills,
                                Kind::Mcp => Page::Mcp,
                                Kind::Instructions => Page::Instructions,
                            };
                        }
                    });
            }
        });
        ui.add_space(24.0);
        ui.label(RichText::new("Kaynak haritası").size(19.0).strong());
        ui.label(RichText::new("Global kaynaklar ve seçili projenin dosyaları birlikte taranır. MCP sunucuları yalnızca bağlantı testi başlatıldığında çalıştırılır.").color(MUTED));
        for (title, detail) in [
            (
                "GLOBAL SKILLS",
                "~/.agents/skills  ·  ~/.codex/skills  ·  ~/.claude/skills  ·  plugin paketleri",
            ),
            (
                "PROJE KAPSAMI",
                ".agents/skills  ·  .claude/skills  ·  .codex/skills  ·  AGENTS.md  ·  .agents/**/*.md",
            ),
            (
                "MCP CONFIG",
                "Codex  ·  Claude Code & Desktop  ·  Cursor  ·  OpenCode",
            ),
        ] {
            ui.add_space(8.0);
            egui::Frame::new()
                .fill(PANEL)
                .corner_radius(8)
                .inner_margin(16)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(RichText::new(title).size(11.0).color(ACCENT));
                    ui.label(detail);
                });
        }
        if !self.ws.warnings.is_empty() {
            ui.add_space(12.0);
            ui.collapsing(
                format!("{} tarama uyarısı", self.ws.warnings.len()),
                |ui| {
                    for warning in &self.ws.warnings {
                        ui.colored_label(Color32::LIGHT_YELLOW, warning);
                    }
                },
            );
        }
    }
    fn resources(&mut self, ui: &mut egui::Ui, kind: Kind) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.scope, 0, "Tümü");
            ui.selectable_value(&mut self.scope, 1, "Global");
            ui.selectable_value(&mut self.scope, 2, "Proje");
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Ad, açıklama, sağlayıcı ara…")
                    .desired_width(240.0),
            );
            if ui.button("+ Yeni").clicked() {
                self.new_resource(kind);
            }
        });
        ui.add_space(8.0);
        let query = self.search.to_lowercase();
        let resources: Vec<_> = self
            .ws
            .resources
            .iter()
            .filter(|r| {
                r.kind == kind
                    && (self.scope == 0 || r.global == (self.scope == 1))
                    && format!(
                        "{} {} {} {}",
                        r.name,
                        r.description,
                        r.provider,
                        r.path.display()
                    )
                    .to_lowercase()
                    .contains(&query)
            })
            .cloned()
            .collect();
        ui.label(
            RichText::new(format!(
                "{} kaynak · detayları açmak için seç",
                resources.len()
            ))
            .color(MUTED)
            .size(12.0),
        );
        if resources.is_empty() {
            ui.add_space(50.0);
            ui.label(RichText::new("Bu kapsamda kaynak bulunamadı.").size(23.0));
            ui.label("Başka bir kapsam seçebilir veya yeni bir dosya oluşturabilirsin.");
        }
        for resource in resources {
            let selected = self
                .editor
                .as_ref()
                .is_some_and(|e| e.resource.path == resource.path);
            egui::Frame::new()
                .fill(if selected {
                    Color32::from_rgb(25, 47, 48)
                } else {
                    PANEL
                })
                .stroke(egui::Stroke::new(
                    1.0_f32,
                    if selected { ACCENT } else { LINE },
                ))
                .corner_radius(10)
                .inner_margin(16)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .selectable_label(
                                selected,
                                RichText::new(&resource.name).size(16.0).strong(),
                            )
                            .clicked()
                        {
                            self.open(resource.clone());
                        }
                        ui.label(
                            RichText::new(if resource.global { "GLOBAL" } else { "PROJE" })
                                .size(10.0)
                                .color(ACCENT),
                        );
                        ui.label(RichText::new(&resource.provider).size(11.0).color(MUTED));
                        if !resource.enabled {
                            ui.colored_label(Color32::LIGHT_YELLOW, "Devre dışı");
                        }
                    });
                    if kind == Kind::Mcp
                        && ui
                            .add_enabled(
                                !self.mcp_tests.busy(),
                                egui::Button::new("Bağlantı testi"),
                            )
                            .clicked()
                    {
                        let cwd = self.ws.settings.active.as_deref().unwrap_or(&self.ws.home);
                        self.mcp_tests.open(&resource.path, cwd);
                    }
                    ui.label(RichText::new(&resource.description).color(MUTED));
                    ui.label(
                        RichText::new(resource.path.to_string_lossy())
                            .monospace()
                            .size(10.0)
                            .color(MUTED),
                    );
                });
            ui.add_space(5.0);
        }
    }
    fn projects(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Projelerini bağla, kapsamı seç.")
                .size(23.0)
                .strong(),
        );
        ui.label(RichText::new("Klasör yolu eklemek yalnızca Kozmos kaydını günceller. Agent dosyaları kaydettiğinde oluşturulur.").color(MUTED));
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.project_input)
                    .hint_text("~/Projects/projem")
                    .desired_width(400.0),
            );
            if ui.button("+ Proje ekle").clicked()
                && self.activate_project(&self.project_input.clone())
            {
                self.project_input.clear();
            }
        });
        ui.add_space(18.0);
        for path in self.ws.settings.projects.clone() {
            egui::Frame::new()
                .fill(PANEL)
                .corner_radius(12)
                .inner_margin(20)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(path.file_name().unwrap_or_default().to_string_lossy())
                                .size(20.0)
                                .strong(),
                        );
                        if self.ws.settings.active.as_ref() == Some(&path) {
                            ui.colored_label(ACCENT, "AKTİF");
                        } else if ui.button("Etkinleştir").clicked() {
                            self.activate_project(&path.to_string_lossy());
                        }
                        if ui
                            .small_button("Listeden çıkar")
                            .on_hover_text("Proje dosyaları silinmez")
                            .clicked()
                        {
                            self.ws.settings.projects.retain(|p| p != &path);
                            if self.ws.settings.active.as_ref() == Some(&path) {
                                self.ws.settings.active =
                                    self.ws.settings.projects.first().cloned();
                            }
                            let result = self.ws.save_settings();
                            self.result(result, "Proje kayıttan çıkarıldı.");
                        }
                    });
                    ui.label(
                        RichText::new(path.to_string_lossy())
                            .color(MUTED)
                            .monospace(),
                    );
                    if !path.is_dir() {
                        ui.colored_label(Color32::LIGHT_YELLOW, "Klasör bulunamadı");
                    }
                });
            ui.add_space(10.0);
        }
        ui.add_space(20.0);
        ui.heading("Özel kaynaklar");
        ui.label(RichText::new("Standart konumların dışındaki skill klasörlerini, MCP dosyalarını ve talimatları envantere ekle.").color(MUTED));
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.source_kind, Kind::Skill, "Skill klasörü");
            ui.selectable_value(&mut self.source_kind, Kind::Mcp, "MCP dosyası");
            ui.selectable_value(&mut self.source_kind, Kind::Instructions, "Talimatlar");
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.source_input)
                    .hint_text("Dosya veya klasör yolu")
                    .desired_width(360.0),
            );
            if ui.button("Kaynak bağla").clicked() {
                let result = self.ws.add_source(&self.source_input, self.source_kind);
                self.result(result, "Özel kaynak envantere eklendi.");
            }
        });
        for source in self.ws.settings.sources.clone() {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "{} · {}",
                    source.kind.label(),
                    source.path.display()
                ));
                if ui.small_button("Bağlantıyı kaldır").clicked() {
                    self.ws
                        .settings
                        .sources
                        .retain(|s| s.path != source.path || s.kind != source.kind);
                    let result = self.ws.save_settings();
                    self.result(result, "Özel kaynak kayıttan çıkarıldı.");
                }
            });
        }
    }
    fn editor_panel(&mut self, ctx: &egui::Context) {
        let Some(mut editor) = self.editor.take() else {
            return;
        };
        let mut close = false;
        egui::SidePanel::right("editor").default_width(500.0).width_range(320.0..=850.0).resizable(true).frame(egui::Frame::new().fill(PANEL).inner_margin(20)).show(ctx, |ui| {
            ui.horizontal(|ui| { ui.label(RichText::new("DOSYA DÜZENLEYİCİ").color(ACCENT).size(11.0)); if editor.dirty() { ui.colored_label(Color32::LIGHT_YELLOW, "Kaydedilmedi"); } });
            ui.heading(&editor.resource.name);
            ui.add(egui::Label::new(RichText::new(editor.resource.path.to_string_lossy()).size(11.0).color(MUTED).monospace()).wrap());
            if editor.resource.kind == Kind::Mcp { ui.label(RichText::new("Tam yapılandırma dosyası. Gizli değerler içeriyor olabilir; içerik yalnızca yerel düzenleyicide gösterilir.").size(11.0).color(MUTED)); }
            ui.horizontal_wrapped(|ui| {
                if ui.button(RichText::new("Kaydet").color(ACCENT)).clicked() {
                    let result = workspace::save_file(&editor.resource.path, editor.original.as_deref(), &editor.content, Some(editor.resource.kind));
                    let success = match &result { Ok(backup) if editor.original.is_some() => format!("Kaydedildi. Yedek: {}", backup.display()), _ => "Yeni kaynak oluşturuldu.".into() };
                    if result.is_ok() { editor.original = Some(editor.content.clone()); }
                    self.result(result, &success);
                }
                if ui.button("Kapat").clicked() { if editor.dirty() { editor.discard = true; } else { close = true; } }
                if ui.button("Yolu kopyala").clicked() { ui.ctx().copy_text(editor.resource.path.to_string_lossy().into_owned()); }
            });
            if editor.discard { ui.horizontal_wrapped(|ui| { ui.colored_label(Color32::LIGHT_YELLOW, "Kaydedilmemiş değişiklikler var."); if ui.button("Değişiklikleri bırak").clicked() { close = true; }
 if ui.button("Düzenlemeye dön").clicked() { editor.discard = false; } }); }
            if editor.resource.kind == Kind::Skill && editor.original.is_some() {
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(!editor.dirty(), egui::Button::new(if editor.resource.enabled { "Devre dışı bırak" } else { "Etkinleştir" })).on_hover_text("SKILL.md dosyasını SKILL.md.disabled olarak yeniden adlandırır. Global değişiklik tüm projeleri etkiler.").clicked() {
                        let result = workspace::toggle_skill(&editor.resource); if result.is_ok() { close = true; } self.result(result, "Skill durumu değiştirildi.");
                    }
                    if editor.resource.global && ui.add_enabled(self.ws.settings.active.is_some() && !editor.dirty(), egui::Button::new("Projeye kopyala")).clicked() && let Some(project) = &self.ws.settings.active { let result = workspace::install_skill(&editor.resource, project); self.result(result, "Skill tüm kaynak dosyalarıyla projeye kopyalandı.");
                    }
                });
            }
            ui.separator();
            egui::ScrollArea::both().id_salt("source_editor").show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut editor.content).font(egui::TextStyle::Monospace).code_editor().desired_width(f32::INFINITY).desired_rows(40));
            });
        });
        if !close {
            self.editor = Some(editor);
        }
    }
    fn create_window(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.create.take() else {
            return;
        };
        let mut open = true;
        let mut prepare = false;
        egui::Window::new("Yeni kaynak oluştur")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(form.kind.label());
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut form.global, true, "Global");
                    ui.selectable_value(&mut form.global, false, "Aktif proje");
                });
                if form.kind == Kind::Skill {
                    ui.add(egui::TextEdit::singleline(&mut form.name).hint_text("skill-adi"));
                }
                if form.kind == Kind::Mcp {
                    ui.horizontal(|ui| {
                        for provider in ["Claude", "Codex", "Cursor"] {
                            ui.selectable_value(&mut form.provider, provider.into(), provider);
                        }
                    });
                }
                match self
                    .ws
                    .target(form.kind, form.global, &form.name, &form.provider)
                {
                    Ok(path) => {
                        ui.label(RichText::new(path.to_string_lossy()).monospace().size(11.0));
                        if path.exists() {
                            ui.label("Bu dosya mevcut; düzenleyicide açılacak.");
                        }
                        prepare = ui.button("Düzenleyicide aç").clicked();
                    }
                    Err(e) => {
                        ui.colored_label(Color32::LIGHT_YELLOW, e);
                    }
                }
                ui.label(
                    RichText::new("Dosya, Kaydet düğmesine bastığında yazılır.")
                        .size(11.0)
                        .color(MUTED),
                );
            });
        if prepare {
            if self.editor.as_ref().is_some_and(Editor::dirty) {
                self.result::<()>(
                    Err("Önce açık dosyayı kaydedin veya değişiklikleri bırakın.".into()),
                    "",
                );
                self.create = Some(form);
                return;
            }
            if let Ok(path) = self
                .ws
                .target(form.kind, form.global, &form.name, &form.provider)
            {
                let resource = Resource {
                    name: if form.kind == Kind::Skill {
                        form.name.clone()
                    } else {
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned()
                    },
                    description: String::new(),
                    path: path.clone(),
                    kind: form.kind,
                    global: form.global,
                    provider: form.provider.clone(),
                    enabled: true,
                };
                if path.exists() {
                    self.open(resource);
                } else {
                    let content = match form.kind { Kind::Skill => format!("---\nname: {}\ndescription: Skill açıklaması\n---\n\n# {}\n\nAgent için talimatları buraya yaz.\n", form.name, form.name), Kind::Instructions => "# Agent talimatları\n\n## Proje\n\n## Çalışma kuralları\n\n## Doğrulama\n".into(), Kind::Mcp if form.provider == "Codex" => "# MCP sunucularını [mcp_servers.sunucu_adi] altında tanımlayın.\n".into(), Kind::Mcp => "{\n  \"mcpServers\": {}\n}\n".into() };
                    self.editor = Some(Editor {
                        resource,
                        original: None,
                        content,
                        discard: false,
                    });
                }
                self.page = match form.kind {
                    Kind::Skill => Page::Skills,
                    Kind::Mcp => Page::Mcp,
                    Kind::Instructions => Page::Instructions,
                };
            }
        } else if open {
            self.create = Some(form);
        }
    }
}
impl AgentOs {
    fn render(&mut self, ctx: &egui::Context) {
        self.sidebar(ctx);
        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::new().fill(PANEL).inner_margin(10))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(if self.failed { "HATA" } else { "KOZMOS" })
                            .color(if self.failed {
                                Color32::LIGHT_RED
                            } else {
                                ACCENT
                            })
                            .size(10.0),
                    );
                    ui.label(RichText::new(&self.message).size(11.0));
                });
            });
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(24, 18)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(self.page.title()).size(26.0).strong());
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Yenile").clicked() {
                            self.ws.refresh();
                            self.message =
                                "Envanter yeniden tarandı. Açık dosyanın içeriği korunuyor.".into();
                            self.failed = false;
                        }
                    });
                });
            });
        self.editor_panel(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(24))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("main_content")
                    .show(ui, |ui| {
                        if let Some(kind) = self.page.kind() {
                            self.resources(ui, kind);
                        } else if self.page == Page::Projects {
                            self.projects(ui);
                        } else {
                            self.overview(ui);
                        }
                    });
            });
        self.create_window(ctx);
        ctx.request_repaint_after(Duration::from_secs(2));
    }
}
impl eframe::App for AgentOs {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.page == Page::System {
            self.sidebar(ctx);
            self.telemetry.sync_project(&self.ws);
            let key = self
                .ws
                .settings
                .active
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "global".into());
            let name = self
                .ws
                .settings
                .project_names
                .entry(key)
                .or_insert_with(|| "Project".into());
            if self.telemetry.show(ctx, name) {
                let result = self.ws.save_settings();
                self.telemetry.report_save(result);
            }
            if let Some(kind) = self.telemetry.open_resource.take() {
                self.scope = 2;
                self.search.clear();
                self.page = match kind {
                    Kind::Skill => Page::Skills,
                    Kind::Mcp => Page::Mcp,
                    Kind::Instructions => Page::Instructions,
                };
            }
            if std::mem::take(&mut self.telemetry.open_projects) {
                self.page = Page::Projects;
            }
            if std::mem::take(&mut self.telemetry.refresh_requested) {
                self.ws.refresh();
            }
        } else {
            self.render(ctx);
        }
        self.mcp_tests.show(ctx);
    }
}
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Kozmos")
            .with_inner_size([1440.0, 920.0])
            .with_min_inner_size([1050.0, 700.0])
            .with_app_id("dev.kozmos.dashboard"),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Kozmos",
        options,
        Box::new(|cc| Ok(Box::new(AgentOs::new(&cc.egui_ctx)))),
    )
}
