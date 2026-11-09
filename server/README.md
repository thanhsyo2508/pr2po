# PR2PO update service

Service Python dùng thư viện chuẩn, chạy trên server Linux để lưu installer đã ký và cung cấp catalog/manifest cho app Tauri. PR và báo giá vẫn lưu cục bộ trong app. Giao diện phát hành được phục vụ tại `/admin/` và chỉ ghi được sau khi đăng nhập.

## Cấu hình server

Chạy service bằng tài khoản Linux riêng, quyền ghi chỉ vào thư mục storage, và chỉ bind vào loopback. Không mở cổng Python trực tiếp ra Internet.

Ví dụ `/etc/pr2po-updates.env`:

```dotenv
PR2PO_UPDATE_BIND=127.0.0.1
PR2PO_UPDATE_PORT=8787
PR2PO_UPDATE_STORAGE=/var/lib/pr2po-updates
PR2PO_UPDATE_PUBLIC_URL=https://clm.tmgtech.io.vn/po2pr
PR2PO_REQUIRE_HTTPS=true
PR2PO_UPDATE_MAX_UPLOAD_MB=512
PR2PO_ADMIN_USER=release-admin
PR2PO_ADMIN_PASSWORD_HASH=pbkdf2_sha256$600000$...$...
PR2PO_ADMIN_SESSION_TTL=7200
```

Tạo password hash trên máy tin cậy, không đưa mật khẩu vào Git hoặc vào app desktop:

```powershell
python server/set_admin_password.py
```

Lệnh hỏi mật khẩu hai lần và in ra dòng `PR2PO_ADMIN_PASSWORD_HASH=...`. Chép dòng hash đó vào env file trên server, đặt username riêng, rồi giới hạn quyền đọc env file cho root/service account. Mật khẩu thô không được ghi vào file hoặc log. Đổi hash để thu hồi mật khẩu cũ; restart service để hủy các phiên đang sống.

Trên server đã cài PR2PO, có thể thiết lập hash trực tiếp mà không lưu mật khẩu thô bằng lệnh tương tác:

```bash
sudo sh -c 'python3 /opt/pr2po-updates/set_admin_password.py >> /etc/pr2po-updates.env'
sudo systemctl restart pr2po-updates
```

Ví dụ Caddy khi domain `clm.tmgtech.io.vn` dành riêng prefix `/po2pr` cho updater:

```caddyfile
clm.tmgtech.io.vn {
    redir /po2pr /po2pr/admin/ 308
    redir /po2pr/ /po2pr/admin/ 308

    handle_path /po2pr/* {
        request_body {
            max_size 512MB
        }
        reverse_proxy 127.0.0.1:8787
    }
    # Giữ nguyên các route hiện có của website tại đây.
}
```

`handle_path` bỏ prefix trước khi chuyển request vào Python. Caddy thiết lập `X-Forwarded-Proto` để server chỉ cho phép thao tác quản trị khi request gốc dùng HTTPS. Giữ nguyên giới hạn body của các route hiện có và chỉ đặt 512 MB cho `/po2pr/*`. Nếu domain đã có reverse proxy hoặc route khác, hãy gắn route mới vào cấu hình hiện tại thay vì thay toàn bộ site. Giao diện quản trị ở `https://clm.tmgtech.io.vn/po2pr/admin/`.

Systemd service cần chạy từ thư mục chứa cả `update_server.py`, `publish_release.py` và thư mục `admin/`. Ví dụ:

```ini
[Unit]
Description=PR2PO signed update service
After=network.target

[Service]
Type=simple
User=pr2po-updates
Group=pr2po-updates
EnvironmentFile=/etc/pr2po-updates.env
WorkingDirectory=/opt/pr2po-updates
ExecStart=/usr/bin/python3 /opt/pr2po-updates/update_server.py
Restart=on-failure
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/pr2po-updates

[Install]
WantedBy=multi-user.target
```

## Phát hành từ giao diện

1. Build gói updater trên máy release có khóa ký riêng. Không tải khóa riêng lên server. Ví dụ NSIS:

   ```powershell
   pnpm run tauri build --bundles nsis
   ```

2. Mở trang HTTPS `/po2pr/admin/`, đăng nhập bằng username và mật khẩu đã cấu hình trên server.
3. Nhập version SemVer, ghi chú phát hành, chọn Windows/x64 và đúng loại bundle. Chọn installer `.exe` của NSIS hoặc `.msi` cùng file chữ ký có tên chính xác `<installer>.sig`.
4. Bấm **Tải lên và phát hành**. Installer và chữ ký được tải vào staging; server kiểm tra tên, loại và dung lượng, sau đó lưu artifact, manifest và cập nhật catalog cuối cùng. Các phiên bản đã công bố là bất biến; phát hành lại cùng version bị từ chối. Dùng version mới khi cần thay gói.

Trang lịch sử hiển thị các version đã công bố. Rollback ở app desktop là chọn/cài lại một version cũ còn được giữ trong catalog; trang quản trị không thay đổi version mới nhất bằng cách xóa release.

## API

- `GET /api/v1/releases`: catalog công khai.
- `GET /api/v1/manifest/{target}/{arch}/{bundle_type}/{version}`: manifest Tauri (`version`, `notes`, `pub_date`, `url`, `signature`); `latest` chọn release mới nhất. `bundle_type=unknown` dùng entry generic `{target}-{arch}` làm fallback.
- `/admin/`: giao diện quản trị.
- `POST /api/v1/admin/login`, `POST .../logout`, `GET .../releases`, `PUT .../artifacts/...`, `POST .../publish`: API quản trị, yêu cầu HTTPS và bearer session. Upload installer/chữ ký bằng raw bytes, sau đó gọi publish bằng metadata JSON.

Đặc tả endpoint, request mẫu, thứ tự build/upload/publish, xử lý lỗi và checklist xác minh nằm ở [`docs/release-publishing.md`](../docs/release-publishing.md). Hãy dùng tài liệu này làm runbook chuẩn thay vì tự đoán API khi phát hành.

Gói cũ không bị ghi đè. Tauri xác minh chữ ký bằng public key đi cùng app trước khi cài; HTTPS và đăng nhập quản trị bảo vệ luồng tải/công bố. Dịch vụ Python hiện dùng session trong RAM, vì vậy restart sẽ yêu cầu đăng nhập lại. Nên đặt thêm giới hạn truy cập trang admin tại reverse proxy/VPN nếu phù hợp với mạng của tổ chức.

## Cấu hình app desktop

`VITE_UPDATE_BASE_URL` trong `.env` là URL mặc định khi người dùng mở hộp Cập nhật lần đầu. Người dùng có thể đổi URL HTTPS ngay trong app; app lưu lựa chọn riêng trên máy và dùng nó cho catalog, kiểm tra bản mới lẫn khôi phục. App chỉ gọi API đọc catalog/manifest; không nhúng tài khoản phát hành. Máy chủ mới vẫn phải phát hành gói ký bằng public key đã tích hợp trong app.

SSH release helper là một luồng riêng, không cần để trang admin hoạt động. Trước khi dùng `server/deploy_release.py`, phải cấu hình `SSH_SERVER_HOST_KEY_SHA256` đã xác minh ngoài băng và `PR2PO_UPDATE_REMOTE_DIR`; không bỏ kiểm tra host key.
