(() => {
  const basePath = window.location.pathname.replace(/\/admin\/?$/, "");
  const adminApi = `${basePath}/api/v1/admin`;
  const $ = (id) => document.getElementById(id);
  let token = "";
  let signedInAs = "";
  let busy = false;
  const loginView = $("login-view");
  const dashboardView = $("dashboard-view");
  const loginMessage = $("login-message");
  const publishMessage = $("publish-message");
  const publishButton = $("publish-button");

  function message(element, text, success = false) {
    element.textContent = text;
    element.classList.toggle("is-success", success);
  }

  async function api(path, options = {}) {
    const headers = new Headers(options.headers || {});
    if (token) headers.set("Authorization", `Bearer ${token}`);
    if (options.body && !(options.body instanceof File)) headers.set("Content-Type", "application/json");
    const response = await fetch(`${adminApi}${path}`, { ...options, headers, cache: "no-store" });
    const payload = response.status === 204 ? null : await response.json().catch(() => null);
    if (!response.ok) throw new Error(payload?.error || `Yêu cầu thất bại (${response.status})`);
    return payload;
  }

  function setBusy(value) {
    busy = value;
    publishButton.disabled = value;
    $("refresh-button").disabled = value;
    $("logout-button").disabled = value;
    publishButton.innerHTML = value ? '<span class="spinner"></span> Đang xử lý…' : 'Tải lên và phát hành <span>→</span>';
  }

  function showDashboard(username) {
    signedInAs = username;
    loginView.classList.add("hidden");
    dashboardView.classList.remove("hidden");
    $("logout-button").classList.remove("hidden");
    document.querySelector(".secure-pill").innerHTML = `<i></i> ${escapeHtml(username)} · phiên bảo mật`;
    void loadReleases();
  }

  function escapeHtml(value) {
    return String(value).replace(/[&<>"']/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[char]);
  }

  async function loadReleases() {
    const list = $("release-list");
    list.replaceChildren(Object.assign(document.createElement("div"), { className: "empty-state", textContent: "Đang tải danh sách…" }));
    try {
      const data = await api("/releases");
      const releases = data?.releases || [];
      if (!releases.length) {
        list.replaceChildren(Object.assign(document.createElement("div"), { className: "empty-state", textContent: "Chưa có phiên bản nào được phát hành." }));
        return;
      }
      const fragment = document.createDocumentFragment();
      for (const release of releases) {
        const item = document.createElement("article"); item.className = "release-item";
        const top = document.createElement("div"); top.className = "release-item-top";
        const version = document.createElement("strong"); version.textContent = `v${release.version}`;
        const date = document.createElement("span"); date.className = "release-date"; date.textContent = formatDate(release.pubDate);
        top.append(version, date);
        const notes = document.createElement("p"); notes.textContent = release.notes || "Không có ghi chú phát hành.";
        const platforms = document.createElement("div"); platforms.className = "platform-list";
        for (const platform of release.platforms || []) {
          const tag = document.createElement("span"); tag.className = "platform-tag"; tag.textContent = platform; platforms.append(tag);
        }
        item.append(top, notes, platforms); fragment.append(item);
      }
      list.replaceChildren(fragment);
    } catch (error) {
      message(publishMessage, `Không tải được danh sách: ${error.message}`);
      list.replaceChildren(Object.assign(document.createElement("div"), { className: "empty-state", textContent: "Không kết nối được dịch vụ." }));
    }
  }

  function formatDate(value) {
    if (!value) return "";
    const date = new Date(value);
    return Number.isNaN(date.getTime()) ? value : new Intl.DateTimeFormat("vi-VN", { dateStyle: "medium" }).format(date);
  }

  function validateVersion(value) { return /^v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(value); }

  function uploadFile(file, kind, metadata, onProgress) {
    return new Promise((resolve, reject) => {
      const xhr = new XMLHttpRequest();
      const parts = [metadata.version, metadata.target, metadata.arch, metadata.bundleType, kind, file.name].map(encodeURIComponent);
      xhr.open("PUT", `${adminApi}/artifacts/${parts.join("/")}`);
      xhr.setRequestHeader("Authorization", `Bearer ${token}`);
      xhr.setRequestHeader("Content-Type", "application/octet-stream");
      xhr.upload.onprogress = (event) => { if (event.lengthComputable) onProgress(Math.round(event.loaded / event.total * 100), file.name); };
      xhr.onload = () => {
        let data = null; try { data = JSON.parse(xhr.responseText); } catch { /* handled below */ }
        if (xhr.status >= 200 && xhr.status < 300) resolve(data);
        else reject(new Error(data?.error || `Upload thất bại (${xhr.status})`));
      };
      xhr.onerror = () => reject(new Error("Mất kết nối trong lúc tải tệp."));
      xhr.onabort = () => reject(new Error("Đã hủy tải tệp."));
      xhr.send(file);
    });
  }

  function progress(title, percent, detail) {
    $("release-progress").classList.remove("hidden");
    $("progress-title").textContent = title;
    $("progress-percent").textContent = `${percent}%`;
    $("progress-bar").value = percent;
    $("progress-detail").textContent = detail || "";
  }

  async function publish() {
    if (busy) return;
    message(publishMessage, "");
    const version = $("version").value.trim();
    const notes = $("notes").value.trim();
    const target = $("target").value;
    const arch = $("arch").value;
    const bundleType = $("bundle-type").value;
    const artifact = $("artifact-file").files[0];
    const signature = $("signature-file").files[0];
    if (!validateVersion(version)) return message(publishMessage, "Nhập phiên bản theo dạng SemVer, ví dụ 0.1.3.");
    if (!notes) return message(publishMessage, "Hãy nhập ghi chú phát hành trước khi tiếp tục.");
    if (!artifact || !signature) return message(publishMessage, "Cần chọn cả installer và file chữ ký .sig.");
    const expectedExtension = bundleType === "nsis" ? ".exe" : ".msi";
    if (!artifact.name.toLowerCase().endsWith(expectedExtension)) return message(publishMessage, `Gói ${bundleType.toUpperCase()} cần installer ${expectedExtension}.`);
    if (signature.name !== `${artifact.name}.sig`) return message(publishMessage, `File chữ ký phải khớp tên installer: ${artifact.name}.sig`);
    if (artifact.size > 512 * 1024 * 1024) return message(publishMessage, "Installer vượt giới hạn 512 MB.");

    const metadata = { version, notes, target, arch, bundleType, artifactName: artifact.name, signatureName: signature.name };
    setBusy(true); $("release-progress").classList.remove("hidden");
    try {
      progress("Đang tải installer…", 0, artifact.name);
      await uploadFile(artifact, "artifact", metadata, (percent, name) => progress("Đang tải installer…", percent, name));
      progress("Đang tải chữ ký…", 0, signature.name);
      await uploadFile(signature, "signature", metadata, (percent, name) => progress("Đang tải chữ ký…", percent, name));
      progress("Đang phát hành…", 100, "Server đang cập nhật manifest và danh mục.");
      const result = await api("/publish", { method: "POST", body: JSON.stringify(metadata) });
      message(publishMessage, `Đã phát hành v${result.version}: ${result.artifact}`, true);
      $("version").value = ""; $("notes").value = ""; $("notes-count").textContent = "0";
      $("artifact-file").value = ""; $("signature-file").value = "";
      $("artifact-name").textContent = "Chưa chọn tệp"; $("signature-name").textContent = "Chưa chọn tệp";
      await loadReleases();
    } catch (error) {
      message(publishMessage, error.message);
    } finally { setBusy(false); }
  }

  $("login-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    if (location.protocol !== "https:") return message(loginMessage, "Đăng nhập chỉ được phép qua HTTPS.");
    const button = $("login-submit"); button.disabled = true; message(loginMessage, "");
    try {
      const response = await fetch(`${adminApi}/login`, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ username: $("username").value.trim(), password: $("password").value }), cache: "no-store" });
      const data = await response.json();
      if (!response.ok) throw new Error(data.error || "Không đăng nhập được.");
      token = data.token; $("password").value = ""; showDashboard(data.username);
    } catch (error) { message(loginMessage, error.message); }
    finally { button.disabled = false; }
  });

  $("logout-button").addEventListener("click", async () => {
    try { await api("/logout", { method: "POST", body: "{}" }); } catch { /* local token is cleared regardless */ }
    token = ""; signedInAs = ""; dashboardView.classList.add("hidden"); loginView.classList.remove("hidden"); $("logout-button").classList.add("hidden");
    message(loginMessage, "Đã đăng xuất.", true); $("username").focus();
  });

  $("publish-button").addEventListener("click", () => void publish());
  $("refresh-button").addEventListener("click", () => { if (!busy) void loadReleases(); });
  $("notes").addEventListener("input", (event) => { $("notes-count").textContent = String(event.target.value.length); });
  $("artifact-file").addEventListener("change", (event) => { $("artifact-name").textContent = event.target.files[0]?.name || "Chưa chọn tệp"; });
  $("signature-file").addEventListener("change", (event) => { $("signature-name").textContent = event.target.files[0]?.name || "Chưa chọn tệp"; });
  $("bundle-type").addEventListener("change", (event) => {
    const nsis = event.target.value === "nsis";
    $("artifact-file").accept = nsis ? ".exe,application/vnd.microsoft.portable-executable" : ".msi,application/x-msi";
    $("artifact-hint").textContent = nsis ? "Chọn file .exe (NSIS)" : "Chọn file .msi";
    $("artifact-file").value = ""; $("artifact-name").textContent = "Chưa chọn tệp";
    $("signature-file").value = ""; $("signature-name").textContent = "Chưa chọn tệp";
  });

  if (location.protocol !== "https:") {
    $("https-warning").classList.remove("hidden");
    $("login-submit").disabled = true;
  }
})();
