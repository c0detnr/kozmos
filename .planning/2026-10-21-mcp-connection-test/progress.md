# Progress

MCP modülü ve UI penceresi eklendi. stdio/HTTP, environment/header çözümlemesi, tek arka plan testi, iptal ve 20 sn süre sınırı hazır. SDK API farkları giderildi. Testler derleniyor.

Tamamlandı: cargo test --locked (24/24), cargo clippy --locked -- -D warnings, cargo fmt -- --check ve cargo build --locked başarılı. UI başsız egui testi, stdio pagination, HTTP auth/boş araç listesi, iptal ve process group temizliği doğrulandı. README kullanım ve sınırlar ile güncellendi. Gerçek kullanıcı MCP sunucuları çalıştırılmadı.
