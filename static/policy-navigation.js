"use strict";
(() => {
  try {
    if (sessionStorage.getItem("skilvella-policy-operation")) document.documentElement.classList.add("policy-restoring");
  } catch { /* 保存時に記録できなければ変更を開始しない。 */ }
  // 履歴キャッシュに古い設定を描画させない。
  window.addEventListener("pagehide", () => document.documentElement.classList.add("policy-restoring"));
  window.addEventListener("pageshow", event => { if (event.persisted) window.dispatchEvent(new Event("skilvella:restore-policies")); });
})();
