# Kozmos çalışma ilkeleri

## Ürün dili ve tasarım

- Kozmos ciddi, profesyonel bir üründür. Tüm çalışmalarda bu standardı koru.
- Arayüzün hiçbir yerinde edebi ifadeler, süslü sloganlar, içi boş iddialar veya gereksiz ürün etiketleri kullanma. “Agent OS”, “Agent Operating System”, “Agent ekosistemin, tek merkezde” ve benzeri metinler ekleme.
- Sidebar’daki Kozmos logosunun altında slogan, sürüm etiketi veya gereksiz açıklama gösterme. Marka alanını sade tut.
- Başlıklar, açıklamalar, araç ipuçları ve durum mesajları kısa, net, somut ve işlev odaklı olsun. Kullanıcının işlem yapmasına veya mevcut durumu anlamasına yardımcı olmayan metni kaldır.
- Ürünün ilk bakışta etkileyici ve merak uyandırıcı olması hedeflenir. Bu etkiyi abartılı metinlerle değil; güçlü görsel hiyerarşi, özenli tasarım, gerçek veriler, anlamlı özellikler ve akıcı etkileşimlerle oluştur.
- Yeni özelliklerde, mevcut ekran değişikliklerinde ve kullanıcıya yönelik proje tanıtımlarında aynı dil ve kalite standardını uygula.

## Başlatma ve proje kapsamı

- Kozmos yalnızca terminalden, üzerinde çalışılacak projenin dizininde `kozmos` komutuyla başlatılır.
- Başlangıçtaki çalışma dizini (`cwd`) oturumun sabit proje köküdür. Proje yolu argümanı, arayüzden proje seçme/değiştirme ve önceki projeyi geri yükleme akışı ekleme.
- Skill, MCP ve agent talimatlarını yalnızca bu proje dizininden oku ve yönet. Ana dizini, global agent yapılandırmalarını veya başka projeleri otomatik tarama.
- Özel kaynaklar ve yeni dosyalar proje kökü içinde kalmalıdır. Proje dışına çıkan sembolik bağlantıları izleme.
- Kozmos ayarlarını `.kozmos/workspace.json` içinde, dışa aktarımları proje dizininde tut. Başka bir proje için kullanıcı terminalde o dizine geçip yeni bir Kozmos oturumu başlatır.
- Arayüzde mevcut proje adını ve dizinini göster; tüm yeni özellikleri bu tek proje çalışma modeline göre tasarla.
