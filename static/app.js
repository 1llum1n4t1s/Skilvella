"use strict";
(() => {
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
  document.addEventListener("submit", event => {
    const form = event.target;
    if (!(form instanceof HTMLFormElement)) return;
    const button = event.submitter;
    if (button?.hasAttribute("data-preview")) return;
    if (form.dataset.submitting === "true") { event.preventDefault(); return; }
    if (dirty && form !== editor && !window.confirm("未保存の本文があります。変更を破棄して操作を続けますか？")) { event.preventDefault(); return; }
    if (form.dataset.confirm && !window.confirm(form.dataset.confirm)) { event.preventDefault(); return; }
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
