// Кнопка «⋯» поверх сайта платформы (как в Android-версии). Работает без доступа к IPC приложения:
// действия переходят по адресам school21app://…, приложение перехватывает такие переходы,
// а список закладок само подставляет в страницу через window.__s21SetBookmarks.
(function () {
  var hosts = ["platform.21-school.ru", "auth.21-school.ru"];
  if (window.top !== window || hosts.indexOf(location.hostname) < 0 || window.__s21Menu) return;
  window.__s21Menu = true;

  var data = { bookmarks: [], start: null };
  var onPlatform = location.hostname === "platform.21-school.ru";

  var icons = {
    peer: '<path d="M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0 1 18 0z"/><circle cx="12" cy="10" r="3"/>',
    settings: '<circle cx="12" cy="12" r="3"/><path d="M12 1v3M12 20v3M4.2 4.2l2.1 2.1M17.7 17.7l2.1 2.1M1 12h3M20 12h3M4.2 19.8l2.1-2.1M17.7 6.3l2.1-2.1"/>',
    browser: '<path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6M15 3h6v6M10 14 21 3"/>',
    pin: '<path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"/>',
    home: '<path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
    x: '<path d="M18 6 6 18M6 6l12 12"/>',
  };
  var actions = [
    ["peer", "Где сидит пир", "#3ddabf"],
    ["settings", "Настройки", "#9aa6b8"],
    ["browser", "Открыть в браузере", "#36c5f0"],
  ];

  // Названия разделов платформы для закладок.
  var sections = {
    "": "Главная", projects: "Проекты", calendar: "Календарь", profile: "Профиль", campus: "Кампус",
    competition: "Соревнования", events: "События", notifications: "Уведомления", progress: "Прогресс",
    activities: "Активности", "skills": "Навыки",
  };

  function svg(name, size) {
    size = size || 18;
    return '<svg width="' + size + '" height="' + size + '" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">' + icons[name] + "</svg>";
  }

  function esc(text) {
    var div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
  }

  /** Понятное имя текущей страницы: раздел + заголовок страницы, если он есть. */
  function pageTitle() {
    var parts = location.pathname.split("/").filter(Boolean);
    var base = sections[parts[0] || ""] || decodeURIComponent(parts[0] || "Страница");
    if (parts.length < 2) return base;
    var heading = document.querySelector("main h1, main h2, h1, h2");
    var text = heading && heading.textContent.trim();
    if (text && text.length <= 60) return base + ": " + text;
    return base + " · " + decodeURIComponent(parts.slice(1).join("/"));
  }

  function go(action, params) {
    location.href = "school21app://" + action + (params ? "?" + new URLSearchParams(params).toString() : "");
  }

  function mount() {
    var host = document.createElement("div");
    host.style.cssText = "position:fixed;right:22px;bottom:22px;z-index:2147483647;";
    var root = host.attachShadow({ mode: "closed" });
    root.innerHTML =
      "<style>" +
      ":host{all:initial}" +
      ".fab{width:46px;height:46px;border-radius:15px;border:none;cursor:pointer;opacity:.88;" +
      "background:linear-gradient(135deg,#36c5f0,#3ddabf,#44eb99);color:#14202c;font:900 22px/1 system-ui;" +
      "box-shadow:0 6px 18px rgba(0,0,0,.35);transition:transform .15s,opacity .15s}" +
      ".fab:hover{opacity:1;transform:translateY(-1px)}" +
      ".menu{position:absolute;right:0;bottom:58px;width:280px;max-height:70vh;overflow:auto;padding:6px;border-radius:18px;" +
      "background:#2c384a;box-shadow:0 12px 32px rgba(0,0,0,.45);font:600 14px/1.3 'Segoe UI',system-ui,sans-serif;color:#fff}" +
      ".menu[hidden]{display:none}" +
      ".label{padding:8px 12px 4px;color:#3ddabf;font-size:11px;letter-spacing:1.1px;text-transform:uppercase}" +
      ".item{display:flex;align-items:center;gap:12px;width:100%;padding:10px 12px;border:none;border-radius:12px;" +
      "background:none;color:#fff;font:inherit;cursor:pointer;text-align:left}" +
      ".item:hover{background:rgba(255,255,255,.06)}" +
      ".item:disabled{opacity:.5;cursor:default;background:none}" +
      ".ic{flex:none;display:grid;place-items:center;width:32px;height:32px;border-radius:50%}" +
      ".bm{display:flex;align-items:center;gap:4px;border-radius:12px}" +
      ".bm:hover{background:rgba(255,255,255,.06)}" +
      ".bm .item{flex:1;min-width:0;padding-right:4px}" +
      ".bm .item span:last-child{overflow:hidden;white-space:nowrap;text-overflow:ellipsis}" +
      ".mini{flex:none;display:grid;place-items:center;width:30px;height:30px;border:none;border-radius:9px;" +
      "background:none;color:#9aa6b8;cursor:pointer}" +
      ".mini:hover{background:rgba(255,255,255,.08);color:#fff}" +
      ".mini.on{color:#44eb99}" +
      ".sep{height:1px;margin:6px 8px;background:rgba(255,255,255,.08)}" +
      ".hint{padding:2px 12px 8px;color:#9aa6b8;font-weight:400;font-size:12px}" +
      "</style>" +
      '<div class="menu" hidden></div>' +
      '<button class="fab" title="School 21">⋯</button>';

    var menu = root.querySelector(".menu");

    function render() {
      var html = "";
      var here = location.href;
      if (data.bookmarks.length) {
        html += '<div class="label">Закладки</div>';
        data.bookmarks.forEach(function (b) {
          var isStart = b.url === data.start;
          html +=
            '<div class="bm">' +
            '<button class="item" data-open="' + esc(b.url) + '"><span class="ic" style="color:#36c5f0;background:#36c5f024">' + svg("pin") + "</span><span>" + esc(b.title) + "</span></button>" +
            '<button class="mini' + (isStart ? " on" : "") + '" data-start="' + esc(b.url) + '" title="' + (isStart ? "Открывается при запуске — нажмите, чтобы отменить" : "Открывать при запуске") + '">' + svg("home", 16) + "</button>" +
            '<button class="mini" data-remove="' + esc(b.url) + '" title="Удалить закладку">' + svg("x", 16) + "</button>" +
            "</div>";
        });
        html += '<div class="hint">⌂ — открывать эту страницу при запуске приложения</div>';
        html += '<div class="sep"></div>';
      }
      if (onPlatform) {
        var pinned = data.bookmarks.some(function (b) { return b.url === here; });
        html += '<button class="item" data-add="1"' + (pinned ? " disabled" : "") + '><span class="ic" style="color:#44eb99;background:#44eb9924">' + svg("pin") + "</span>" +
          (pinned ? "Страница уже в закладках" : "Закрепить эту страницу") + "</button>";
        html += '<div class="sep"></div>';
      }
      actions.forEach(function (a) {
        html += '<button class="item" data-action="' + a[0] + '"><span class="ic" style="color:' + a[2] + ";background:" + a[2] + '24">' + svg(a[0]) + "</span>" + a[1] + "</button>";
      });
      menu.innerHTML = html;
    }

    window.__s21SetBookmarks = function (next) {
      data = next || { bookmarks: [], start: null };
      render();
    };

    root.querySelector(".fab").addEventListener("click", function (e) {
      e.stopPropagation();
      if (menu.hidden) render();
      menu.hidden = !menu.hidden;
    });
    menu.addEventListener("click", function (e) {
      e.stopPropagation();
      var el = e.target.closest("button");
      if (!el || el.disabled) return;
      menu.hidden = true;
      if (el.dataset.open) location.href = el.dataset.open;
      else if (el.dataset.start) go("bookmark/start", { url: el.dataset.start === data.start ? "" : el.dataset.start });
      else if (el.dataset.remove) go("bookmark/remove", { url: el.dataset.remove });
      else if (el.dataset.add) go("bookmark/add", { url: location.href, title: pageTitle() });
      else if (el.dataset.action) go(el.dataset.action);
    });
    document.addEventListener("click", function () {
      menu.hidden = true;
    });
    render();
    document.documentElement.appendChild(host);
  }

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", mount);
  else mount();
})();
