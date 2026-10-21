# Kozmos

Skill, MCP ve agent talimatlarını global veya proje kapsamında yöneten, sistem telemetrisi de sunan Rust masaüstü uygulaması.

## Çalıştır

```sh
cargo run --locked
```

Derlenmiş uygulama: `./target/debug/kozmos`

Geçerli klasörü açmak için `cargo run --locked -- .` veya `kozmos .` kullanın (ikili PATH içindeyse). Başka bir proje için klasör yolunu argüman olarak verin. Logonun altındaki proje seçici, kayıtlı projeleri son açılma sırasıyla gösterir; seçim uygulama yeniden açıldığında korunur.

## MCP bağlantı testi

**MCP bağlantıları → yapılandırma kartında Bağlantı testi → sunucuda Test et** yolunu izle.

- Pencere açılırken kayıtlı dosya okunur ve her sunucu ayrı test edilir; düzenleyicideki kaydedilmemiş değişiklikler kullanılmaz.
- stdio komutları ve Streamable HTTP desteklenir. Codex TOML, Claude/Cursor `mcpServers` ve OpenCode `mcp` yapılandırmaları okunur.
- Test MCP el sıkışmasını yapar, sunucu kimliğini ve araç listesini getirir; hiçbir araç çağırmaz. Araç listeleme yeteneği olmayan sunucular da bağlanabilir.
- Bağlantı sonucu, test süresi, araç adları/açıklamaları ve hata özeti gösterilir. Her test 20 saniyeyle sınırlıdır ve iptal edilebilir; pencereyi kapatmak da testi iptal eder. Arayüz çalışmaya devam eder.
- Yerel komut yalnızca **Test et** ile başlatılır. Seçili proje çalışma klasörü olarak kullanılır (proje yoksa ana dizin); `cwd` ile değiştirilebilir. Test sonunda başlatılan süreç grubu kapatılır.
- `env` / `environment`, HTTP `headers` / `http_headers`, `env_http_headers` ve `bearer_token_env_var` desteklenir. `${VAR}`, `${VAR:-varsayılan}` ve `{env:VAR}` ifadeleri Kozmos oturumunun ortamından çözülür.
- Eksik komut, erişim/yetkilendirme, protokol ve zaman aşımı hataları gösterilir; ham hata yanıtları ve kimlik bilgileri hata metinlerine eklenmez.
- Eski SSE taşıması ve OAuth giriş akışı desteklenmez. HTTP bağlantısı yapılandırılan sunucuya gider; Kozmos ayrı bir telemetri servisine veri göndermez.

## Proje dashboardı ve sistem telemetrisi

- Aktif proje klasörü, projeye ait skill/MCP yapılandırması/talimat sayıları ve kaynak yönetimi kısayolları
- Ayrı alt bölümde bilgisayar ve donanım bilgileri
- Her saniye güncellenen gerçek CPU, RAM, ağ ve sistem çalışma süresi
- Son 90 örneğin işlemci ve bellek grafikleri
- Tam genişlikte animasyonlu uzay bannerı: hareketli gezegen ufku, derinlik katmanlı yıldızlar ve aralıklı meteor geçişleri
- Banner üzerindeki Duraklat/Hareket düğmesi tüm sahne animasyonunu durdurur/sürdürür
- Banner üzerinde tıklanarak düzenlenen, proje bazında saklanan ad; Enter kaydeder, Escape iptal eder
- Belleğe göre sıralanan ilk 6 süreç
- Başlatma, duraklatma ve sıfırlama destekli 25 dakikalık odak sayacı
- Ana dizine `kozmos-<zaman>.csv` olarak grafik örneklerini kaydetme

Soldaki menüden kontrol merkezi, kaynaklar, çalışma alanları ve proje dashboardı açılır. Dashboard tek, kaydırılabilir sayfadır. Boşluk tuşu, metin düzenlenmiyorken banner hareketini durdurur/sürdürür; telemetri devam eder. Disk bilgisi uygulama açılırken kök dosya sistemi için okunur. Ağ değeri loopback dışındaki arayüzlerin toplamıdır; sanal arayüzler dahil olabilir. Henüz örnek alınmadığında ilk saniye değerler sıfır görünebilir.

Kaynak envanteri yerel çalışır; MCP bağlantıları yalnızca kullanıcı test başlattığında kurulur. Kozmos telemetri göndermez; yönetici izni istemez. Grafik arayüz [egui / eframe](https://github.com/emilk/egui) ile çizilir; tarayıcı kullanmaz. Linux `/proc` dosya sistemi ve disk bilgisi için `df` gereklidir. Wayland veya X11, OpenGL/EGL destekli bir masaüstü oturumu gerekir.

## Doğrulama

MCP stdio testleri yerel fixture için `python3` ve `sleep` kullanır. Testler gerçek yapılandırılmış sunucuları başlatmaz; HTTP fixture yalnızca loopback üzerinde çalışır.

```sh
cargo test --locked
cargo clippy --locked -- -D warnings
cargo fmt -- --check
```
