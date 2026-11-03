"use strict";
(() => {
  document.querySelectorAll("[data-mcp-address]").forEach(element => {
    element.textContent = `${window.location.origin}/mcp`;
  });
  const editor = document.querySelector("form[data-editor]");
  let dirty = editor?.dataset.dirty === "true";
  let saving = false;
  editor?.addEventListener("input", () => {
    dirty = true;
    const status = document.getElementById("edit-state");
    if (status) status.textContent = "未保存の変更";
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
  confirmationTitle.textContent = "操作の確認";
  const confirmationMessage = document.createElement("p");
  confirmationMessage.id = "confirmation-message";
  const confirmationActions = document.createElement("div");
  confirmationActions.className = "confirmation-actions";
  const cancelButton = document.createElement("button");
  cancelButton.type = "button";
  cancelButton.textContent = "キャンセル";
  cancelButton.autofocus = true;
  const confirmButton = document.createElement("button");
  confirmButton.type = "button";
  confirmButton.className = "primary";
  confirmButton.textContent = "続行する";
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
        if (await askConfirmation("未保存の本文があります。変更を破棄してページを移動しますか？")) {
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
      if (dirty && form !== editor) messages.push("未保存の本文があります。変更を破棄して操作を続けますか？");
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
    if (label) label.textContent = `${count}件選択`;
    const submit = document.getElementById("bulk-submit");
    if (submit) submit.disabled = count === 0;
    if (selectAll) { selectAll.checked = count > 0 && count === checks.length; selectAll.indeterminate = count > 0 && count < checks.length; }
  }
  checks.forEach(input => input.addEventListener("change", selectionChanged));
  selectAll?.addEventListener("change", () => { checks.forEach(input => { input.checked = selectAll.checked; }); selectionChanged(); });
  document.body.addEventListener("htmx:responseError", () => {
    const preview = document.getElementById("preview");
    if (preview) preview.textContent = "プレビューを表示できませんでした。本文は編集欄に保持されています。";
  });
})();
