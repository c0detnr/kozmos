# Kozmos — Geliştirme ve Yeni Oturuma Devir Planı

Tarih: 21 Ekim 2026
Proje dizini: /home/mami/Projects/kozmos

## Amaç

Kozmos, Linux üzerinde çalışan yerel bir Rust masaüstü uygulamasıdır. Hedefi; farklı yapay zekâ araçlarının skill paketlerini, MCP yapılandırmalarını ve agent talimatlarını genel veya proje kapsamında tek merkezden yönetmektir.

İlk sürüm CPU, RAM, ağ, süreçler ve odak sayacı içeren bir sistem izleme uygulamasıydı. Sonraki çalışma bunu “Agent OS” çalışma alanı yöneticisine dönüştürdü. Sistem izleme özellikleri korunuyor.

## Yeni oturumda başlangıç

1. Bu dosyayı, varsa geçerli AGENTS.md talimatlarını ve mevcut kaynak kodunu oku.
2. `.planning/2026-10-17-agent-os/` ve `.planning/2026-10-17-kozmos-rust-gui/` notlarını incele. Eski planın işaretlenmemiş kutuları tek başına işin yapılmadığı anlamına gelmez; kodu doğrula.
3. Aşağıdaki mevcut durum bilgisini kodla karşılaştır.
4. Önce Aşama 0'ı tamamla; sonra ilk tamamlanmamış uygulama aşamasını uçtan uca ilerlet.
5. Her aşama sonunda bu dosyayı ve `progress.md` dosyasını güncelle. Sonuçları, kararları ve sıradaki somut adımı kaydet.

Bu dosya bir yol haritasıdır. Kullanıcı önerilen özellikler arasından tek tek kapsam seçimi yapmadı; aşağıdaki sıra önceki konuşmada önerilen önceliklere dayanır. Ayrıntılı davranışları mevcut mimariye göre belirle; ürünü önemli ölçüde değiştiren belirsizlikleri kullanıcıyla netleştir.

## Doğrulanmış mevcut durum

- Cargo paketi: kozmos 0.2.0, Rust edition 2024.
- Arayüz: eframe/egui 0.33; Glow, Wayland ve X11.
- Kaynaklar: `src/main.rs` (Agent OS arayüzü), `src/workspace.rs` (keşif ve dosya işlemleri), `src/telemetry.rs` (eski sistem izleme özellikleri).
- Skill keşfi, düzenleme, oluşturma, etkinleştirme/devre dışı bırakma ve genel skill'i projeye kopyalama kodda var.
- Genel ve projeye özel MCP yapılandırmalarını keşfetme ve düzenleme kodda var. JSON/TOML doğrulaması var. MCP komutları keşif sırasında çalıştırılmıyor.
- AGENTS.md / CLAUDE.md gibi agent talimatlarının keşfi ve düzenlenmesi var.
- Proje kaydı, aktif proje seçimi ve özel kaynak yolları ekleme var.
- Kayıttan önce yedekleme, dışarıdan değişmiş dosyaya yazmayı reddetme ve atomik yayımlama işlemleri var.
- 21 Ekim 2026 tarihinde `cargo test --locked`: 12 test geçti, 0 başarısız.
- Güncel Agent OS arayüzünün görsel ve uçtan uca doğrulamasının tamamlandığı teyit edilmedi.
- README.md eski sistem izleme sürümünü anlatıyor; güncel ürünü yansıtmıyor.
- Son incelemede proje dizini Git deposu değildi.
- Önceki oturum hafıza araması başarısız oldu; mevcut durum kaynak kodu ve yerel çalışma notlarından çıkarıldı.

## Çalışma yöntemi

- Her aşama kullanılabilir, incelenebilir bir sonuç üretsin. Bütün yol haritasını tek büyük değişiklikte uygulama.
- Mevcut özellikleri yeniden yazmadan önce incele.
- Gerekli bağımsız araştırma, test açığı analizi ve kod incelemesinde alt ajanlar kullanılabilir. Kullanıcı temiz oturumda alt ajanlarla çalışma yöntemini gündeme getirdi; görevleri açıkça böl ve aynı dosyaya eşzamanlı yazma görevi verme.
- Ana ajan entegrasyon, test ve nihai sonucun sorumlusudur.
- Kullanıcının gerçek agent ayarlarını test verisi olarak değiştirme; geçici dizinler kullan.
- Testler gerçek hata durumlarını ve kullanıcı davranışını doğrulasın.
- Her aşama sonunda ilgili testleri çalıştır; tamamlandı kutusunu ancak kabul kriterleri doğrulandıysa işaretle.

## Aşama 0 — Mevcut sürümü sağlamlaştır

- [ ] Mevcut kaynakları ve uygulama akışlarını doğrula; eski notlarla farkları kaydet.
- [ ] Git yoksa yerel depo oluştur, uygun .gitignore ile mevcut durumu başlangıç kaydına al. Gizli değerleri ve derleme çıktılarını dahil etme; uzak depoya yayınlama bu aşamanın kapsamında değil.
- [ ] Uygulamayı aç; gezinme, proje seçme, kaynak arama, düzenleme ve oluşturma akışlarını geçici bir proje üzerinde kontrol et.
- [ ] README'yi güncel amaç, özellikler, dosya işlemleri ve çalıştırma adımlarıyla güncelle.
- [ ] Kalıcı mimari özeti, geliştirme kuralları ve doğrulama komutlarını kısa bir AGENTS.md dosyasına yaz; mevcut talimat varsa koruyarak güncelle.
- [ ] Güncel sonuçları progress.md dosyasına kaydet.

Kabul: Güncel ürün belgelenmiş, temel akışlar doğrulanmış ve başlangıç durumu geri dönülebilir biçimde kayıt altına alınmış olsun.

## Aşama 1 — Kaydetmeden önce değişiklik farkı

- [ ] Mevcut dosya içeriği ile düzenlenmiş içerik arasındaki satır farkını arayüzde göster.
- [ ] Eklenen ve silinen satırlar anlaşılır olsun; yeni dosya ve boş içerik durumlarını ele al.
- [ ] Mevcut dış değişiklik kontrolünü, MCP doğrulamasını ve yedeklemeyi koru.
- [ ] Gerçek davranışı doğrulayan uygun kontrolleri ekle.

Kabul: Kullanıcı diske yazmadan önce değişecek satırları görebilsin. Dışarıdan değişen dosya sessizce ezilmesin. Gizli değer içerebilen farklar loglara yazılmasın.

## Aşama 2 — Yedek geçmişi ve geri yükleme

Bağımlılık: Aşama 1'deki fark görünümü geri yükleme önizlemesinde kullanılabilir.

- [ ] Mevcut .kozmos-*.bak yedeklerini ilgili dosyayla ilişkilendirip tarih sırasıyla göster.
- [ ] Seçilen yedeği mevcut içerikle karşılaştır.
- [ ] Geri yükleme öncesinde mevcut dosyanın da yedeğini al.
- [ ] Mevcut izin, sembolik bağlantı ve dış değişiklik korumalarıyla uyumlu geri yükle.
- [ ] Eksik/okunamayan yedek ve geçersiz MCP içeriği durumlarını ele al.

Kabul: Kullanıcı önceki sürüme dönebilsin ve geri yüklemeden önceki sürüm de korunmuş olsun. Eski yedekleri otomatik silme bu aşamada yok.

## Aşama 3 — Yapılandırma sağlık kontrolü

- [ ] Kontrol kapsamını açıkça tanımla: JSON/TOML sözdizimi, beklenen MCP alanlarının yapısı, açık dosya yolu referansları ve komut bulunabilirliği.
- [ ] Statik kontroller ile gerçek bağlantı testini ayır. Henüz çalıştırılmamış sunucuyu “bağlantı başarılı” diye işaretleme.
- [ ] Genel ve proje kapsamı ile kaynak dosyasını bulgu yanında göster.
- [ ] Hata ve uyarıları anlaşılır açıklama ve ilgili dosyaya erişimle sun.
- [ ] Sağlık taramasında MCP komutu çalıştırma ve dosyaları otomatik değiştirme.

Kabul: Geçici örneklerde bozuk yapılandırma ve bulunamayan komut tespit edilsin; geçerli örnekler doğru sınıflandırılsın. Çözümlenemeyen ortam değişkenlerini kesin hata gibi raporlama.

## Aşama 4 — Tekrar ve çakışma tespiti

- [ ] Aynı skill'in farklı konumlardaki kopyalarını tespit et; aynı ada sahip olmayı tek başına kesin özdeşlik sayma.
- [ ] Aynı MCP adının birden fazla kaynakta bulunmasını göster.
- [ ] Genel ve projeye özel tanımların kapsamını göster; araçlara göre öncelik kuralları doğrulanmadıysa kesin üstünlük iddiasında bulunma.
- [ ] Otomatik silme veya birleştirme yapmadan karşılaştırma sun.

Kabul: Kullanıcı tekrarları ve olası çakışmaları kaynaklarıyla görebilsin; mevcut dosyalar tarama sonucunda değişmesin.

## Aşama 5 — Proje kurulum sihirbazı

Bağımlılık: Mevcut oluşturma/kopyalama işlemleri, fark görünümü ve çakışma kontrolleri.

- [ ] Proje ve hedef araç seçimi, skill seçimi, MCP/talimat şablonları için aşamalı akış oluştur.
- [ ] Yazılacak dosyaları işlemden önce göster.
- [ ] Mevcut dosya çakışmalarını açıkça çöz; sessiz üzerine yazma yapma.
- [ ] Çok dosyalı işlemlerde hata olduğunda kısmi durumun nasıl ele alınacağını tasarla ve doğrula.

Kabul: Boş geçici projede seçilen kaynaklar doğru yerlere kurulsun; dolu projede mevcut içerik korunarak çakışmalar görünür olsun.

## Aşama 6 — Kullanıcının başlattığı MCP bağlantı testi

- [ ] Desteklenecek taşıma türlerini ve sağlayıcıları önce belirle; mevcut MCP yapılandırma biçimlerini araştır.
- [ ] Kullanıcının seçtiği sunucu için açık bir test eylemi sun. Keşif veya sağlık taraması sırasında otomatik çalıştırma.
- [ ] Bağlantı, başlangıç el sıkışması ve araç listesi sonucunu göster.
- [ ] Zaman aşımı, iptal ve başlatılan alt süreçlerin temizlenmesini uygula.
- [ ] Test için araç çağrısı yürütme; gizli değerleri ve hassas çıktıları loglardan ayıkla.

Kabul: Kontrollü test sunucusunda başarı, bağlantı hatası, zaman aşımı ve iptal durumları doğrulansın; sahipsiz alt süreç kalmasın.

## Sonraki ürün havuzu — Henüz ayrıntılı uygulama kapsamı değil

Aşağıdakiler öneri havuzudur. İlk aşamalar tamamlandıktan sonra kullanıcıyla önceliklendir.

- [ ] Git deposundan skill kurma; kaynak ve sürüm kaydı.
- [ ] Skill güncelleme kontrolü; yerel değişikliklerle karşılaştırma.
- [ ] Frontend/backend/araştırma gibi çalışma profilleri.
- [ ] Yeniden kullanılabilir proje şablonları.
- [ ] Dosya değişiklikleri ve kurulumlar için işlem geçmişi.
- [ ] Gizli değerleri maskeleme ve uygun gizli değer saklama yaklaşımı.
- [ ] Klavyeden erişilen arama ve komut paleti.
- [ ] Gizli değerleri ayıklayarak içe/dışa aktarma.
- [ ] Projeye bağlı agent çalıştırma, çıktı ve durum paneli.
- [ ] Agent görevleri, sonuçları ve bekleyen onaylar için görev panosu.

## Doğrulama komutları

```sh
cargo test --locked
cargo clippy --locked -- -D warnings
cargo fmt -- --check
cargo run --locked
```

Arayüz değişikliklerinde ilgili akışı uygulamada da kontrol et. Çalıştırılamayan veya görsel olarak doğrulanamayan kontrolleri açıkça kaydet; yapılmış gibi raporlama.

## Oturum sonu devir kaydı

progress.md dosyasında şunlar bulunsun:

- Tamamlanan aşama ve kullanıcı açısından sonuç.
- Değişen dosyalar ve önemli tasarım kararları.
- Çalıştırılan kontroller ve sonuçları.
- Bilinen sorunlar ve çözülmemiş sorular.
- Sıradaki tek somut başlangıç adımı.

## Temiz oturum için örnek başlangıç promptu

> plan.md dosyasını, varsa progress.md ve AGENTS.md dosyalarını oku. Mevcut kodla doğrula ve ilk tamamlanmamış aşamayı uçtan uca uygula. Bağımsız araştırma ve inceleme için gerektiğinde alt ajan kullan; aynı dosyalara eşzamanlı yazma görevi verme. İlgili testleri ve kontrolleri çalıştır, bulguları gider, planı ve ilerleme kaydını güncelle. Önceki sohbetin bilindiğini varsayma.
