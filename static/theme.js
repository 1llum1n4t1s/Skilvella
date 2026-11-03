"use strict";
(() => {
  // CSSを読み込む前に保存済みの選択を適用する。自動追従はCSSのメディアクエリに任せる。
  const key = "skilvella.theme";
  const valid = value => ["auto", "light", "dark"].includes(value) ? value : "auto";
  let preference = "auto";
  try { preference = valid(localStorage.getItem(key)); } catch {}
  document.documentElement.dataset.theme = preference;

  document.addEventListener("DOMContentLoaded", () => {
    const selector = document.querySelector("[data-theme-selector]");
    if (!selector) return;
    selector.value = preference;
    selector.disabled = false;
    selector.addEventListener("change", () => {
      preference = valid(selector.value);
      document.documentElement.dataset.theme = preference;
      try { localStorage.setItem(key, preference); } catch {}
    });
  });
})();
