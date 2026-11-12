"use strict";
(() => {
  const languageSelector = document.querySelector("[data-language-selector]");
  // 旧版のブラウザー設定は、最初のGETで同じ接続先のCookieへ引き継ぐ。
  const languageCookie = document.body.dataset.languageCookie;
  if (languageCookie) {
    try {
      const oldLanguage = localStorage.getItem("skilvella.language");
      const legacyChoices = ["auto", "ja", "en", "zh-CN", "zh-TW", "ko", "es", "fr", "de", "pt-BR"];
      const cookiePrefix = languageCookie + "=";
      const readLanguageCookie = () => document.cookie.split(";").map(cookie => cookie.trim()).find(cookie => cookie.startsWith(cookiePrefix));
      if (legacyChoices.includes(oldLanguage) && !legacyChoices.includes(readLanguageCookie()?.slice(cookiePrefix.length))) {
        document.cookie = cookiePrefix + oldLanguage + "; Max-Age=31536000; Path=/; SameSite=Lax";
        if (readLanguageCookie() === cookiePrefix + oldLanguage) {
          try { localStorage.removeItem("skilvella.language"); } catch {}
          if (oldLanguage !== "auto" && (oldLanguage !== document.documentElement.lang || languageSelector)) {
            window.location.reload();
            return;
          }
        }
      }
    } catch { /* ストレージを利用できない場合も自動選択で表示する。 */ }
  }
  const messages = JSON.parse(document.body.dataset.clientMessages);
  const t = (id, values = {}) => (Object.hasOwn(messages, id) ? messages[id] : id).replace(/\{(\w+)\}/g, (token, name) =>
    Object.hasOwn(values, name) ? values[name] : token);
  languageSelector?.addEventListener("change", () => languageSelector.form.requestSubmit());
  const operationKey = "skilvella-policy-operation";
  async function refreshPolicyDisplay() {
    if (document.querySelector("form[data-editor]")) {
      const form = document.querySelector('form[action$="/codex"]');
      if (form) {
        const response = await fetch(form.action.replace(/\/codex$/, ""), { cache: "no-store" });
        if (!response.ok) throw new Error("Cannot refresh policy");
        const latest = new DOMParser().parseFromString(await response.text(), "text/html").querySelector('form[action$="/codex"]');
        if (!latest) throw new Error("Cannot refresh policy");
        form.replaceWith(latest);
      }
      document.documentElement.classList.remove("policy-restoring");
      showRecoveryNotice();
    } else if (document.querySelector(".codex-policy-form") || window.location.pathname === "/" || window.location.pathname === "/settings") {
      window.location.replace(window.location.href);
    } else {
      document.documentElement.classList.remove("policy-restoring");
      showRecoveryNotice();
    }
  }
  window.addEventListener("skilvella:restore-policies", () => {
    void (async () => {
      // 復元された文書でも記録を読み直す。
      let record;
      try { record = sessionStorage.getItem(operationKey); } catch { await refreshPolicyDisplay(); return; }
      if (record) await recoverPolicyOperation();
      else await refreshPolicyDisplay();
    })().catch(() => { document.getElementById("policy-recovery-status").textContent = t("policy-unconfirmed"); });
  });
  async function recoverPolicyOperation() {
      try {
        const recovery = JSON.parse(sessionStorage.getItem(operationKey));
        if (!recovery || typeof recovery.id !== "string") throw new Error("Invalid operation");
        const deadline = Date.now() + 30000;
        let recoveryNotice = "";
        let csrf;
        if (!recovery.complete) {
          const settings = await fetch("/settings", { cache: "no-store" });
          if (!settings.ok) throw new Error("Missing token");
          csrf = new DOMParser().parseFromString(await settings.text(), "text/html").querySelector('input[name="csrf"]')?.value;
          if (!csrf) throw new Error("Missing token");
        }
        while (!recovery.complete) {
          const response = await fetch("/settings/policy-operations/" + encodeURIComponent(recovery.id), { method: "POST", body: new URLSearchParams({ csrf }), cache: "no-store" });
          if (!response.ok) throw new Error("Cannot confirm save");
          const status = await response.json();
          if (status.state === "cancelled" || status.state === "failed") { recoveryNotice = "policy-recovered"; break; }
          if (status.state === "sync_failed") { recoveryNotice = "policy-sync-failed"; break; }
          if (status.state === "complete") break;
          if (Date.now() >= deadline) throw new Error("Cannot confirm save");
          await new Promise(resolve => setTimeout(resolve, 500));
        }
        sessionStorage.removeItem(operationKey);
        if (recoveryNotice) sessionStorage.setItem("skilvella-policy-cancelled", recoveryNotice);
        await refreshPolicyDisplay();
      } catch {
        document.getElementById("policy-recovery-status").textContent = t("policy-unconfirmed");
      }
  }
  if (document.documentElement.classList.contains("policy-restoring")) void recoverPolicyOperation();
  function showRecoveryNotice() {
  try {
    if (sessionStorage.getItem("skilvella-policy-cancelled")) {
      const message = sessionStorage.getItem("skilvella-policy-cancelled");
      sessionStorage.removeItem("skilvella-policy-cancelled");
      const notice = document.createElement("div");
      notice.className = "notice warning";
      notice.setAttribute("role", "alert");
      notice.textContent = t(message);
      document.querySelector("main").prepend(notice);
    }
  } catch { /* ストレージを利用できない場合、保存操作側で停止する。 */ }
  }
  showRecoveryNotice();
  document.querySelectorAll("[data-mcp-address]").forEach(element => {
    element.textContent = `${window.location.origin}/mcp`;
  });
  const editor = document.querySelector("form[data-editor]");
  let dirty = editor?.dataset.dirty === "true";
  let saving = false;
  editor?.addEventListener("input", () => {
    dirty = true;
    const status = document.getElementById("edit-state");
    if (status) status.textContent = t("dirty");
  });
  window.addEventListener("beforeunload", event => {
    if (dirty && !saving) { event.preventDefault(); event.returnValue = ""; }
  });
  const confirmationDialog = document.createElement("dialog");
  confirmationDialog.className = "confirmation-dialog";
  confirmationDialog.setAttribute("aria-labelledby", "confirmation-title");
  confirmationDialog.setAttribute("aria-describedby", "confirmation-message");
  const confirmationTitle = document.createElement("h2");
  confirmationTitle.id = "confirmation-title";
  confirmationTitle.textContent = t("confirm-title");
  const confirmationMessage = document.createElement("p");
  confirmationMessage.id = "confirmation-message";
  const confirmationActions = document.createElement("div");
  confirmationActions.className = "confirmation-actions";
  const cancelButton = document.createElement("button");
  cancelButton.type = "button";
  cancelButton.textContent = t("cancel");
  cancelButton.autofocus = true;
  const confirmButton = document.createElement("button");
  confirmButton.type = "button";
  confirmButton.className = "primary";
  confirmButton.textContent = t("continue");
  confirmationActions.append(cancelButton, confirmButton);
  confirmationDialog.append(confirmationTitle, confirmationMessage, confirmationActions);
  document.body.append(confirmationDialog);
  cancelButton.addEventListener("click", () => confirmationDialog.close("cancel"));
  confirmButton.addEventListener("click", () => confirmationDialog.close("confirm"));
  confirmationDialog.addEventListener("cancel", event => {
    event.preventDefault();
    confirmationDialog.close("cancel");
  });
  function askConfirmation(message) {
    const previousFocus = document.activeElement;
    confirmationMessage.textContent = message;
    confirmationDialog.returnValue = "";
    return new Promise(resolve => {
      confirmationDialog.addEventListener("close", () => {
        if (previousFocus instanceof HTMLElement && previousFocus.isConnected) previousFocus.focus();
        resolve(confirmationDialog.returnValue === "confirm");
      }, { once: true });
      confirmationDialog.showModal();
    });
  }
  const approvedSubmissions = new WeakMap();
  let pendingConfirmation = null;
  async function confirmSubmission(form, button, message) {
    pendingConfirmation = form;
    try {
      if (!await askConfirmation(message) || !form.isConnected) return;
      if (button && (!button.isConnected || button.form !== form || button.disabled)) return;
      approvedSubmissions.set(form, { button });
      // requestSubmitで検証を再実行し、元の送信ボタンのname/valueも維持する。
      if (button) form.requestSubmit(button);
      else form.requestSubmit();
    } finally {
      approvedSubmissions.delete(form);
      pendingConfirmation = null;
    }
  }
  document.addEventListener("click", event => {
    if (!dirty || saving || event.defaultPrevented || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
    const link = event.target instanceof Element ? event.target.closest("a[href]") : null;
    if (!link || link.hasAttribute("download") || (link.target && link.target !== "_self")) return;
    const destination = new URL(link.href, window.location.href);
    if (destination.origin !== window.location.origin) return;
    if (destination.pathname === window.location.pathname && destination.search === window.location.search && destination.hash !== window.location.hash) return;
    event.preventDefault();
    if (pendingConfirmation) return;
    pendingConfirmation = link;
    void (async () => {
      try {
        if (await askConfirmation(t("discard-navigation"))) {
          saving = true;
          window.location.assign(destination.href);
        }
      } finally {
        pendingConfirmation = null;
      }
    })();
  });
  document.addEventListener("submit", event => {
    const form = event.target;
    if (!(form instanceof HTMLFormElement)) return;
    const button = event.submitter;
    if (button?.hasAttribute("data-preview")) return;
    if (form.dataset.submitting === "true") { event.preventDefault(); return; }
    const approval = approvedSubmissions.get(form);
    approvedSubmissions.delete(form);
    const approved = approval && approval.button === button;
    if (!approved) {
      if (pendingConfirmation) { event.preventDefault(); return; }
      const messages = [];
      if (dirty && form !== editor) messages.push(t("discard-operation"));
      const confirmation = button?.dataset.confirm || form.dataset.confirm;
      if (confirmation) messages.push(confirmation + (form.dataset.confirmCount ? `\n${t("eligible")} ${form.dataset.confirmCount}` : ""));
      if (messages.length) {
        event.preventDefault();
        void confirmSubmission(form, button, messages.join("\n\n"));
        return;
      }
    }
    if (form.id === "bulk-form") {
      const selected = [...document.querySelectorAll(".skill-select:checked")].map(input => input.value);
      if (!selected.length) { event.preventDefault(); return; }
      document.getElementById("selected-ids").value = selected.join(",");
    }
    if (form.id === "repository-bulk-form" && !repositorySelectionChanged()) { event.preventDefault(); return; }
    form.dataset.submitting = "true";
    saving = true;
    // 送信ボタンの値をブラウザーが取得してから無効にする。
    window.setTimeout(() => {
      for (const control of form.querySelectorAll("button[type=submit]")) {
        control.disabled = true;
        if (control.dataset.busy) { control.dataset.originalLabel = control.textContent; control.textContent = control.dataset.busy; }
      }
    }, 0);
  });
  window.addEventListener("pageshow", () => {
    saving = false;
    for (const form of document.querySelectorAll("form[data-submitting]")) {
      delete form.dataset.submitting;
      for (const button of form.querySelectorAll("button[type=submit]")) { button.disabled = false; if (button.dataset.originalLabel) button.textContent = button.dataset.originalLabel; }
    }
    selectionChanged();
    repositorySelectionChanged();
  });
  const checks = [...document.querySelectorAll(".skill-select")];
  const selectAll = document.getElementById("select-all");
  let policyPending = false;
  let policyUncertain = false;
  function selectionChanged() {
    const count = checks.filter(input => input.checked).length;
    const label = document.getElementById("selection-count");
    if (label) label.textContent = t("selected", { count });
    const submit = document.getElementById("bulk-submit");
    if (submit) submit.disabled = count === 0 || policyPending || policyUncertain;
    if (selectAll) { selectAll.checked = count > 0 && count === checks.length; selectAll.indeterminate = count > 0 && count < checks.length; }
  }
  checks.forEach(input => input.addEventListener("change", selectionChanged));
  selectAll?.addEventListener("change", () => { checks.forEach(input => { input.checked = selectAll.checked; }); selectionChanged(); });
  const policyForms = [...document.querySelectorAll(".codex-policy-form")];
  let policyNavigation = null;
  document.addEventListener("click", event => {
    if (!policyPending || event.button !== 0 || event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return;
    const link = event.target instanceof Element ? event.target.closest("a[href]") : null;
    if (!link || link.hasAttribute("download") || (link.target && link.target !== "_self")) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    policyNavigation ??= () => { window.location.assign(link.href); };
  }, true);
  document.addEventListener("submit", event => {
    if (!policyPending || !(event.target instanceof HTMLFormElement)) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    const form = event.target;
    const button = event.submitter;
    policyNavigation ??= () => { if (button?.isConnected && !button.disabled) form.requestSubmit(button); else form.requestSubmit(); };
  }, true);
  function policyStatus(form, key) {
    form.dataset.policyStatus = key;
    form.querySelector(".codex-policy-status").textContent = t(key);
  }
  policyForms.forEach(form => {
    form.addEventListener("change", async event => {
      if (!(event.target instanceof HTMLInputElement) || event.target.name !== "mode") return;
      if (policyPending) return;
      const selected = event.target;
      const hadFocus = form.contains(document.activeElement);
      const body = new URLSearchParams(new FormData(form));
      const operation = crypto.randomUUID();
      try {
        sessionStorage.setItem(operationKey, JSON.stringify({ id: operation, complete: false }));
      } catch {
        policyUncertain = true;
        form.querySelectorAll('input[name="mode"]').forEach(input => { input.checked = false; });
        policyForms.forEach(item => { item.querySelector("fieldset").disabled = true; });
        document.getElementById("policy-uncertain-warning").hidden = false;
        selectionChanged();
        policyStatus(form, "policy-unconfirmed");
        return;
      }
      body.set("operation", operation);
      policyPending = true;
      policyForms.forEach(item => { item.querySelector("fieldset").disabled = true; });
      selectionChanged();
      form.setAttribute("aria-busy", "true");
      policyStatus(form, "policy-saving");
      let uncertain = false;
      let syncFailed = false;
      try {
        const response = await fetch(form.action, { method: "POST", headers: { Accept: "application/json" }, body, keepalive: true });
        // 次の文書は完了後にもう一度取得する。古いGET応答との競合も防ぐ。
        const recorded = JSON.parse(sessionStorage.getItem(operationKey));
        if (!response.ok) throw new Error("Policy update failed");
        const sync = await response.json();
        if (typeof sync.enabled !== "boolean" || typeof sync.last_error !== "string") throw new Error("Invalid sync status");
        if (!sync.last_error && recorded?.id === operation) sessionStorage.setItem(operationKey, JSON.stringify({ id: operation, complete: true }));
        syncFailed = !!sync.last_error;
        const warning = document.getElementById("codex-sync-warning");
        if (warning) {
          warning.querySelector("#codex-sync-error").textContent = sync.last_error;
          warning.hidden = !sync.last_error;
        }
        policyForms.forEach(item => {
          if (item !== form && item.dataset.policyStatus && item.dataset.policyUncertain !== "true") {
            delete item.dataset.policyStatus;
            item.querySelector(".codex-policy-status").textContent = "";
          }
        });
        policyStatus(form, sync.last_error ? "policy-sync-failed" : !sync.enabled ? "policy-disabled" : "policy-saved");
      } catch {
        uncertain = true;
        policyUncertain = true;
        form.dataset.policyUncertain = "true";
        form.querySelectorAll('input[name="mode"]').forEach(input => { input.checked = false; });
        policyStatus(form, "policy-unconfirmed");
        document.getElementById("policy-uncertain-warning").hidden = false;
      } finally {
        const next = policyNavigation;
        policyNavigation = null;
        policyPending = false;
        policyForms.forEach(item => { item.querySelector("fieldset").disabled = policyUncertain; });
        selectionChanged();
        form.removeAttribute("aria-busy");
        if (!uncertain && hadFocus && (document.activeElement === document.body || document.activeElement === document.documentElement)) selected.focus({ preventScroll: true });
        if (!uncertain && !syncFailed && next) next();
      }
    });
  });
  // 一覧の「状態」列: 公開・非公開のラジオを選んだ時点で保存する。
  // 失敗したら選択を元に戻し、理由を行内に表示する（公開時はサーバーがファイルを再検証する）。
  document.querySelectorAll(".publish-form").forEach(form => {
    const fieldset = form.querySelector("fieldset");
    const status = form.querySelector(".publish-status");
    let saved = form.querySelector('input[name="published"]:checked')?.value ?? "";
    const showStatus = (text, failed = false) => {
      status.textContent = text;
      status.classList.toggle("error-text", failed);
    };
    form.addEventListener("change", async event => {
      if (!(event.target instanceof HTMLInputElement) || event.target.name !== "published") return;
      const value = event.target.value;
      if (value === saved) return;
      const body = new URLSearchParams(new FormData(form));
      fieldset.disabled = true;
      form.setAttribute("aria-busy", "true");
      showStatus(t("policy-saving"));
      try {
        const response = await fetch(form.action, { method: "POST", headers: { Accept: "application/json" }, body, keepalive: true });
        let result = {};
        try { result = await response.json(); } catch { /* 下で失敗として扱う。 */ }
        if (!response.ok || typeof result.published !== "boolean") throw new Error(typeof result.error === "string" ? result.error : `HTTP ${response.status}`);
        saved = value;
        const warning = document.getElementById("codex-sync-warning");
        if (warning && typeof result.last_error === "string") {
          warning.querySelector("#codex-sync-error").textContent = result.last_error;
          warning.hidden = !result.last_error;
        }
        showStatus(t(result.last_error ? "policy-sync-failed" : "policy-saved"));
      } catch (error) {
        // 保存できなかった選択は残さず、保存済みの状態へ戻す。
        form.querySelectorAll('input[name="published"]').forEach(input => { input.checked = input.value === saved; });
        showStatus(t("publish-failed") + (error.message ? " " + error.message : ""), true);
      } finally {
        fieldset.disabled = false;
        form.removeAttribute("aria-busy");
      }
    });
  });
  const repositoryForm = document.getElementById("repository-bulk-form");
  function repositorySelectionChanged() {
    if (!repositoryForm) return false;
    const selected = [...repositoryForm.querySelectorAll(".repository-check:checked:not(:disabled)")];
    const count = selected.reduce((sum, input) => sum + Number(input.dataset.count), 0);
    const label = document.getElementById("repository-selection-count");
    label.textContent = t("repositories-selected", { groups: selected.length, count });
    label.hidden = false;
    const valid = selected.length > 0 && [...repositoryForm.querySelectorAll("select")].some(select => select.value !== "");
    document.getElementById("repository-bulk-submit").disabled = !valid || repositoryForm.dataset.submitting === "true";
    return valid;
  }
  repositoryForm?.addEventListener("change", repositorySelectionChanged);
  repositorySelectionChanged();
  document.querySelectorAll("button[data-preview]").forEach(button => {
    const form = button.form;
    const preview = document.getElementById("preview");
    if (!form || !preview) return;
    const indicator = form.querySelector("[data-preview-status]");
    let running = false;
    let queued = false;
    async function updatePreview() {
      if (running) { queued = true; return; }
      running = true;
      preview.setAttribute("aria-busy", "true");
      if (indicator) indicator.hidden = false;
      try {
        do {
          queued = false;
          try {
            const response = await fetch(button.formAction, {
              method: "POST",
              mode: "same-origin",
              credentials: "same-origin",
              body: new URLSearchParams(new FormData(form))
            });
            if (!response.ok) throw new Error("Preview request failed");
            preview.innerHTML = await response.text();
          } catch {
            preview.textContent = t("preview-failed");
          }
        } while (queued && button.isConnected);
      } finally {
        running = false;
        preview.removeAttribute("aria-busy");
        if (indicator) indicator.hidden = true;
      }
    }
    // クリックは必須入力の検証をせずプレビューし、通常のフォーム送信は残す。
    button.addEventListener("click", event => {
      event.preventDefault();
      void updatePreview();
    });
  });

  // 「＋ フォルダーを追加」「参照…」: Skilvella本体にOS標準のフォルダー選択ダイアログを開かせる。
  // ブラウザーからは絶対パスを取得できないため、選んだパスはサーバーから受け取る。
  // ダイアログを開けない環境（SSH接続など）では、パスの直接入力欄を開いて案内する。
  document.querySelectorAll("[data-folder-browse]").forEach(button => {
    const form = button.form;
    const input = form?.querySelector("[data-folder-path]");
    const csrf = form?.querySelector('input[name="csrf"]')?.value;
    if (!form || !input || !csrf) return;
    const manual = form.querySelector("[data-folder-manual]");
    const status = form.querySelector("[data-folder-status]");
    button.hidden = false;
    // ダイアログで選べる場合は直接入力欄を畳む（入力済みの値が戻されたときは開いたままにする）。
    if (manual && !input.value && !status?.textContent) manual.open = false;
    const showStatus = key => { if (status) status.textContent = key ? t(key) : ""; };
    button.addEventListener("click", async () => {
      if (button.disabled || form.dataset.submitting === "true") return;
      button.disabled = true;
      button.setAttribute("aria-busy", "true");
      showStatus("folder-dialog-opening");
      try {
        const response = await fetch("/dialog/folder", {
          method: "POST",
          headers: { Accept: "application/json" },
          body: new URLSearchParams({ csrf }),
          cache: "no-store",
        });
        let result = {};
        try { result = await response.json(); } catch { /* 下で未対応として扱う。 */ }
        if (typeof result.path === "string" && result.path) {
          input.value = result.path;
          showStatus("");
          // 追加フォームは選んだ時点で登録まで進める。場所の変更は内容を確認してから送信する。
          if (button.hasAttribute("data-submit")) form.requestSubmit();
          else input.focus();
          return;
        }
        if (result.cancelled) { showStatus(""); return; }
        showStatus(result.busy ? "folder-dialog-busy" : "folder-dialog-unavailable");
      } catch {
        showStatus("folder-dialog-unavailable");
      } finally {
        button.disabled = false;
        button.removeAttribute("aria-busy");
      }
      if (manual) manual.open = true;
      input.focus();
    });
  });

  const updates = document.querySelector("[data-updates]");
  if (updates) {
    const checkButton = updates.querySelector("[data-update-check]");
    const installButton = updates.querySelector("[data-update-install]");
    const releaseLink = updates.querySelector("[data-update-release]");
    const status = updates.querySelector("[data-update-status]");
    let busy = false;
    let info = null;
    function updateStatus(key, values = {}, detail = "") {
      status.textContent = t(key, values) + (detail ? " " + detail : "");
    }
    async function requestUpdate(action) {
      const response = await fetch(`/settings/updates/${action}`, {
        method: "POST",
        body: new URLSearchParams({ csrf: updates.dataset.csrf }),
        headers: { Accept: "application/json" },
        // 適用中は通信だけを打ち切って失敗表示にしない。サーバー側の結果を待つ。
        signal: action === "check" ? AbortSignal.timeout(45000) : undefined,
      });
      let result;
      try { result = await response.json(); } catch { throw new Error(`HTTP ${response.status}`); }
      if (!response.ok) throw new Error(typeof result.error === "string" ? result.error : `HTTP ${response.status}`);
      return result;
    }
    async function checkForUpdates() {
      if (busy) return;
      busy = true;
      info = null;
      checkButton.disabled = true;
      installButton.hidden = true;
      releaseLink.hidden = true;
      updateStatus("update-checking");
      try {
        const result = await requestUpdate("check");
        if (typeof result.available !== "boolean" || typeof result.supported !== "boolean" || typeof result.latest_version !== "string") throw new Error("Invalid update response");
        info = result;
        updateStatus(result.available ? "update-available" : "update-current", { version: result.latest_version });
        if (!result.supported) updateStatus("update-unsupported");
        installButton.hidden = !(result.available && result.supported);
        installButton.disabled = false;
        const release = new URL(result.release_url);
        if (release.origin === "https://github.com" && release.pathname.startsWith("/1llum1n4t1s/Skilvella/releases/")) {
          releaseLink.href = release.href;
          releaseLink.hidden = false;
        }
      } catch (error) {
        info = null;
        installButton.hidden = true;
        updateStatus("update-check-failed", {}, error.message);
      } finally {
        busy = false;
        checkButton.disabled = false;
      }
    }
    checkButton.addEventListener("click", () => { void checkForUpdates(); });
    installButton.addEventListener("click", () => {
      if (busy || !info?.available || !info.supported || pendingConfirmation) return;
      pendingConfirmation = installButton;
      void (async () => {
        try {
          if (!await askConfirmation(t("update-confirm"))) return;
          busy = true;
          checkButton.disabled = true;
          installButton.disabled = true;
          updateStatus("update-downloading");
          await requestUpdate("install");
          updateStatus("update-restarting");
        } catch (error) {
          info = null;
          installButton.hidden = true;
          busy = false;
          checkButton.disabled = false;
          updateStatus("update-install-failed", {}, error.message);
        } finally {
          pendingConfirmation = null;
        }
      })();
    });
    if (new URLSearchParams(window.location.search).get("check_updates") === "1") {
      const location = new URL(window.location.href);
      location.searchParams.delete("check_updates");
      window.history.replaceState(null, "", location);
      void checkForUpdates();
    }
  }
})();
