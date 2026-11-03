"use strict";
(() => {
  const t = (key, values) => window.skilvellaI18n?.t(key, values) ?? key;
  document.querySelectorAll("[data-mcp-address]").forEach(element => {
    element.textContent = `${window.location.origin}/mcp`;
  });
  const editor = document.querySelector("form[data-editor]");
  let dirty = editor?.dataset.dirty === "true";
  let saving = false;
  editor?.addEventListener("input", () => {
    dirty = true;
    const status = document.getElementById("edit-state");
    if (status) status.textContent = t("未保存の変更");
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
  confirmationTitle.textContent = t("操作の確認");
  const confirmationMessage = document.createElement("p");
  confirmationMessage.id = "confirmation-message";
  const confirmationActions = document.createElement("div");
  confirmationActions.className = "confirmation-actions";
  const cancelButton = document.createElement("button");
  cancelButton.type = "button";
  cancelButton.textContent = t("キャンセル");
  cancelButton.autofocus = true;
  const confirmButton = document.createElement("button");
  confirmButton.type = "button";
  confirmButton.className = "primary";
  confirmButton.textContent = t("続行する");
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
    confirmationTitle.textContent = t("操作の確認");
    cancelButton.textContent = t("キャンセル");
    confirmButton.textContent = t("続行する");
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
        if (await askConfirmation(t("未保存の本文があります。変更を破棄してページを移動しますか？"))) {
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
      if (dirty && form !== editor) messages.push(t("未保存の本文があります。変更を破棄して操作を続けますか？"));
      const confirmation = button?.dataset.confirm || form.dataset.confirm;
      if (confirmation) messages.push(confirmation);
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
  });
  const checks = [...document.querySelectorAll(".skill-select")];
  const selectAll = document.getElementById("select-all");
  function selectionChanged() {
    const count = checks.filter(input => input.checked).length;
    const label = document.getElementById("selection-count");
    if (label) label.textContent = t("{count}件選択", { count });
    const submit = document.getElementById("bulk-submit");
    if (submit) submit.disabled = count === 0;
    if (selectAll) { selectAll.checked = count > 0 && count === checks.length; selectAll.indeterminate = count > 0 && count < checks.length; }
  }
  document.addEventListener("skilvella:language", selectionChanged);
  checks.forEach(input => input.addEventListener("change", selectionChanged));
  selectAll?.addEventListener("change", () => { checks.forEach(input => { input.checked = selectAll.checked; }); selectionChanged(); });
  document.body.addEventListener("htmx:responseError", () => {
    const preview = document.getElementById("preview");
    if (preview) preview.textContent = t("プレビューを表示できませんでした。本文は編集欄に保持されています。");
  });

  const updates = document.querySelector("[data-updates]");
  if (updates) {
    const checkButton = updates.querySelector("[data-update-check]");
    const installButton = updates.querySelector("[data-update-install]");
    const releaseLink = updates.querySelector("[data-update-release]");
    const status = updates.querySelector("[data-update-status]");
    let busy = false;
    let info = null;
    let statusKey = "更新を確認できます。";
    let statusValues = {};
    let diagnostic = "";
    function updateStatus(key, values = {}, detail = "") {
      statusKey = key;
      statusValues = values;
      diagnostic = detail;
      status.textContent = t(key, values) + (detail ? " " + detail : "");
    }
    document.addEventListener("skilvella:language", () => updateStatus(statusKey, statusValues, diagnostic));
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
      updateStatus("更新を確認中…");
      try {
        const result = await requestUpdate("check");
        if (typeof result.available !== "boolean" || typeof result.supported !== "boolean" || typeof result.latest_version !== "string") throw new Error("Invalid update response");
        info = result;
        updateStatus(result.available ? "バージョン {version} を利用できます。" : "最新バージョンです。", { version: result.latest_version });
        if (!result.supported) updateStatus("この環境では自動更新を利用できません。リリースページを確認してください。");
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
        updateStatus("更新の確認に失敗しました。接続を確認して再試行してください。", {}, error.message);
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
          if (!await askConfirmation(t("更新のインストールを開始しますか？ アプリを再起動します。"))) return;
          busy = true;
          checkButton.disabled = true;
          installButton.disabled = true;
          updateStatus("更新をダウンロードして検証中…");
          await requestUpdate("install");
          updateStatus("更新を開始しました。アプリの再起動をお待ちください。");
        } catch (error) {
          info = null;
          installButton.hidden = true;
          busy = false;
          checkButton.disabled = false;
          updateStatus("更新をインストールできませんでした。再度更新を確認してください。", {}, error.message);
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
