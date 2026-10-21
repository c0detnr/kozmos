MCP kaynakları tam yapılandırma dosyası olarak tutuluyor. Codex TOML, Claude/Cursor JSON mcpServers ve OpenCode JSON mcp alanları mevcut. UI egui; arka plan işçisi ve kanal ile sonuç aktarılacak. SDK belgeleri: https://docs.rs/rmcp/latest/rmcp/transport/index.html

Uygulama rmcp 3.6.0 istemcisi kullanıyor. HTTP redirect kapalı; ham sunucu hataları gösterilmiyor. OAuth/legacy SSE desteklenmiyor. Sunucu test süreci kendi process group içinde çalışıyor; iptal/zaman aşımı/normal bitişte grup temizleniyor. UI işçi thread kapanışı iptal sinyalini ve join işlemini tamamlıyor.
