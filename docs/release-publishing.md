# Phát hành bản cập nhật PR2PO

Tài liệu này là quy trình chuẩn để phát hành gói cho updater trên `clm.tmgtech.io.vn`. Release GitHub và catalog updater trên server là hai nơi riêng: muốn app tự cập nhật thì phải phát hành gói ký lên updater server.

## Quy trình nhanh

1. Tăng cùng một version SemVer trong `package.json`, `src-tauri/Cargo.toml` và `src-tauri/tauri.conf.json`; đồng bộ lockfile nếu công cụ build cập nhật nó.
2. Kiểm tra `.env` cục bộ có `TAURI_SIGNING_PRIVATE_KEY` trỏ đúng khóa updater. Không commit `.env`, không gửi private key lên server hay GitHub.
3. Build đúng loại bundle; mỗi lệnh phát hành tạo installer và chữ ký cạnh nó:

   ```powershell
   pnpm run build
   pnpm run tauri build --bundles nsis
   pnpm run tauri build --bundles msi
   ```

   NSIS tạo `PR2PO_<version>_x64-setup.exe` và `.exe.sig`; MSI tạo `PR2PO_<version>_x64_en-US.msi` và `.msi.sig`. Không đổi nội dung installer sau khi ký.

4. Mở `https://clm.tmgtech.io.vn/po2pr/admin/`, đăng nhập tài khoản phát hành, chọn version, `windows`, `x86_64`, bundle type tương ứng; nhập notes; chọn đúng installer và file `<tên installer>.sig`; bấm **Tải lên và phát hành**. Phải phát hành từng bundle riêng.
5. Kiểm tra release trong trang quản trị, sau đó kiểm tra API public theo phần **Xác minh** bên dưới.
6. Nếu cần GitHub Release, tạo tag/release và đính kèm installer, portable, `.sig` phù hợp cùng checksum riêng. GitHub Release không tự cập nhật catalog của updater server.

### Quy tắc bất biến

- Mỗi `version + target + arch + bundle type` chỉ phát hành một lần. Nếu build sai, tăng version mới; không thay installer đã phát hành.
- Phát hành lại cùng version với notes khác bị từ chối. Chữ ký phải có tên chính xác `<artifact>.sig`.
- Server công bố asset và manifest trước, cập nhật catalog sau. Catalog là danh sách version mà client nhìn thấy.
- Phiên bản cũ được giữ để có thể khôi phục. Không xóa thủ công file release hoặc sửa catalog để rollback.
- `generic` là fallback cho app Tauri có `bundle_type=unknown`; thường trỏ tới installer NSIS đã ký. API hỗ trợ `generic`, nhưng form admin hiện chỉ có `nsis`/`msi`. Muốn tạo generic, gọi API trực tiếp với `bundleType=generic`, dùng lại installer NSIS và chữ ký của chính installer đó; không gửi bundle type `unknown` để publish.

## API phát hành

Base URL:

```text
https://clm.tmgtech.io.vn/po2pr
```

Các API quản trị chỉ chạy qua HTTPS. Token là bearer session tạm thời, lưu trong RAM server và hết hạn theo `PR2PO_ADMIN_SESSION_TTL` (mặc định 7200 giây). Không ghi token hoặc mật khẩu vào log, file source hay chat.

### 1. Đăng nhập

```http
POST /api/v1/admin/login
Content-Type: application/json

{"username":"<admin-user>","password":"<admin-password>"}
```

Thành công `200`:

```json
{"token":"<session-token>","expiresIn":7200,"username":"<admin-user>"}
```

Gửi `Authorization: Bearer <session-token>` cho các route admin tiếp theo.

### 2. Xem các version đã phát hành

```http
GET /api/v1/admin/releases
Authorization: Bearer <session-token>
```

Trả `200` với `releases[]`; mỗi release có `version`, `notes`, `pubDate` và `platforms[]`.

### 3. Upload installer và chữ ký

Gửi raw bytes (`application/octet-stream`) bằng `PUT` cho từng file:

```http
PUT /api/v1/admin/artifacts/{version}/{target}/{arch}/{bundle_type}/artifact/{filename}
PUT /api/v1/admin/artifacts/{version}/{target}/{arch}/{bundle_type}/signature/{filename}.sig
Authorization: Bearer <session-token>
Content-Type: application/octet-stream
```

Giá trị hợp lệ thường dùng: `target=windows`, `arch=x86_64`, `bundle_type=nsis` hoặc `msi`. Encode từng segment của URL nếu tên có ký tự đặc biệt. Giới hạn mặc định 512 MB cho installer và 32 KB cho chữ ký. Thành công `201` trả `{name, bytes, sha256}`. Upload lỗi chưa phát hành version vào catalog.

### 4. Publish sau khi cả hai file upload thành công

```http
POST /api/v1/admin/publish
Authorization: Bearer <session-token>
Content-Type: application/json

{
  "version": "0.1.4",
  "target": "windows",
  "arch": "x86_64",
  "bundleType": "nsis",
  "artifactName": "PR2PO_0.1.4_x64-setup.exe",
  "signatureName": "PR2PO_0.1.4_x64-setup.exe.sig",
  "notes": "Ghi chú phiên bản"
}
```

Thay `bundleType` và tên file nếu publish MSI. Thành công `201` trả `{version, artifact, sha256}`. Server xác minh metadata, file đã upload và chữ ký không rỗng trước khi ghi manifest/catalog. Không gửi private key vào API.

### PowerShell end-to-end cho một bundle

Đoạn dưới đây hỏi credential trong hộp thoại, upload đúng cặp installer/`.sig`, publish rồi kiểm tra manifest và URL tải. Chạy riêng một lần cho NSIS và một lần cho MSI; generic dùng `BundleType='generic'` với đúng installer NSIS và `.sig` tương ứng.

```powershell
$Base = 'https://clm.tmgtech.io.vn/po2pr'
$Version = '0.1.4'
$BundleType = 'nsis' # nsis, msi, hoặc generic
$Artifact = (Resolve-Path '.\PR2PO_0.1.4_x64-setup.exe').Path
$Signature = "$Artifact.sig"
$Notes = 'Ghi chú phiên bản'
if (-not (Test-Path -LiteralPath $Signature)) { throw "Thiếu chữ ký: $Signature" }

$Credential = Get-Credential -Message 'Đăng nhập PR2PO release admin'
$LoginBody = @{ username = $Credential.UserName; password = $Credential.GetNetworkCredential().Password } | ConvertTo-Json
$Session = Invoke-RestMethod -Uri "$Base/api/v1/admin/login" -Method Post -ContentType 'application/json' -Body $LoginBody
$Headers = @{ Authorization = "Bearer $($Session.token)" }
$ArtifactName = [IO.Path]::GetFileName($Artifact)
$SignatureName = [IO.Path]::GetFileName($Signature)
$TargetPath = "$Version/windows/x86_64/$BundleType"

try {
    foreach ($File in @(@{ Kind = 'artifact'; Path = $Artifact; Name = $ArtifactName }, @{ Kind = 'signature'; Path = $Signature; Name = $SignatureName })) {
        $EncodedName = [uri]::EscapeDataString($File.Name)
        $Url = "$Base/api/v1/admin/artifacts/$TargetPath/$($File.Kind)/$EncodedName"
        Invoke-RestMethod -Uri $Url -Method Put -Headers $Headers -ContentType 'application/octet-stream' -InFile $File.Path | Out-Null
    }

    $Metadata = @{
        version = $Version; target = 'windows'; arch = 'x86_64'; bundleType = $BundleType
        artifactName = $ArtifactName; signatureName = $SignatureName; notes = $Notes
    } | ConvertTo-Json
    Invoke-RestMethod -Uri "$Base/api/v1/admin/publish" -Method Post -Headers $Headers -ContentType 'application/json' -Body $Metadata

    $Manifest = Invoke-RestMethod -Uri "$Base/api/v1/manifest/windows/x86_64/$BundleType/latest"
    if ($Manifest.version -ne $Version -or -not $Manifest.signature) { throw 'Manifest version/signature check failed.' }
    Invoke-WebRequest -Uri $Manifest.url -Method Head -UseBasicParsing | Select-Object StatusCode, Headers
}
finally {
    if ($Session.token) { Invoke-RestMethod -Uri "$Base/api/v1/admin/logout" -Method Post -Headers $Headers -ContentType 'application/json' -Body '{}' | Out-Null }
}
```

Thay đường dẫn artifact, version và notes trước khi chạy. Không paste mật khẩu/token vào script hoặc transcript PowerShell đã chia sẻ.

### 5. Đăng xuất

```http
POST /api/v1/admin/logout
Authorization: Bearer <session-token>
Content-Type: application/json

{}
```

### Mã lỗi cần biết

- `400`: metadata sai, thiếu chữ ký hoặc version đã được phát hành; đọc trường `error`.
- `401`: token sai/hết hạn; đăng nhập lại.
- `413`: payload vượt giới hạn cấu hình.
- `426`: request admin không đi qua HTTPS.
- `429`: đăng nhập sai quá nhiều lần; chờ 10 phút.
- `503`: admin/password hash chưa cấu hình trên server.

## Xác minh sau publish

Đọc catalog:

```powershell
$base = 'https://clm.tmgtech.io.vn/po2pr'
Invoke-RestMethod "$base/api/v1/releases"
```

Đọc manifest mới nhất của NSIS hoặc MSI:

```powershell
Invoke-RestMethod "$base/api/v1/manifest/windows/x86_64/nsis/latest"
Invoke-RestMethod "$base/api/v1/manifest/windows/x86_64/msi/latest"
```

Manifest hợp lệ cần có `version`, `notes`, `pub_date`, URL HTTPS trỏ tới asset và `signature` không rỗng. Mở URL asset để chắc file tải được. Nếu app chạy từ dev/portable gửi `bundle_type=unknown`, server phải trả về manifest generic; nếu manifest không có, client có thể báo `Could not fetch a valid release JSON from the remote`.

Chỉ kiểm tra phát hiện bản mới trước; cài update sẽ đóng/khởi động lại app. Sau khi cài, mở trang **Cập nhật** và xác nhận version hiện tại. Rollback dùng danh sách version cũ trong app, không xóa release hiện tại trên server.

## Release checklist cho người thực hiện / AI

- [ ] Xác nhận branch, version chưa dùng, và working tree; không lấy file trong `dataset/` hoặc `outputs/` làm source trừ khi đó là artifact cần đính kèm.
- [ ] Xác nhận ba version file thống nhất; build và kiểm tra `.sig` tương ứng với đúng installer.
- [ ] Publish từng bundle bằng HTTPS; không đưa password, session token hoặc updater private key vào Git, lệnh được log, hoặc nội dung báo cáo.
- [ ] Xác nhận catalog, manifest `nsis`, `msi`, `unknown`/generic cần dùng, và URL tải file.
- [ ] Nếu tạo GitHub Release, kiểm tra tag trỏ đúng commit và mọi asset tải được; ghi rõ `.sig` là chữ ký updater Tauri, không khẳng định nó là Authenticode.
- [ ] Không chạy installer/update trên máy release nếu chưa được yêu cầu; cập nhật có thể thoát hoặc thay thế app đang mở.
