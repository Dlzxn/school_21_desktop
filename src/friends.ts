import { invoke } from "@tauri-apps/api/core";
import { $, errorText, escapeHtml, getInfo, openPlatform, openWindow, place, type Settings, type Workstation } from "./common";
import { icon, renderIcons } from "./icons";

interface FriendInfo {
  login: string;
  workstation: Workstation | null;
  level: number | null;
  failed: boolean;
}

let settings: Settings;
let friends: FriendInfo[] = [];
let loading = false;

function row(f: FriendInfo | { login: string; pending: true }): string {
  const info = "pending" in f ? null : f;
  const ws = info?.workstation ?? null;
  const status = !info
    ? `<span class="muted">…</span>`
    : info.failed
      ? `<span class="muted">Не удалось загрузить</span>`
      : ws
        ? `<span class="on-text">${escapeHtml(place(ws))}</span>`
        : `<span class="muted">Сейчас не в кампусе</span>`;
  const level = info?.level != null ? `<span class="muted small">уровень ${info.level}</span>` : "";
  return `<div class="row clickable friend" data-login="${escapeHtml(f.login)}">
      <div class="avatar sm${ws ? " online" : ""}">${escapeHtml(f.login.charAt(0).toUpperCase())}</div>
      <div class="body">
        <div class="label">${escapeHtml(f.login)} ${level}</div>
        <div class="hint">${status}</div>
      </div>
      <button class="mini" data-remove="${escapeHtml(f.login)}" title="Убрать из друзей">${icon("x", 16)}</button>
    </div>`;
}

function render(pending: string[] = []) {
  const all = [...friends, ...pending.map((login) => ({ login, pending: true as const }))];
  // В кампусе — наверху; дальше в порядке добавления.
  const sorted = [...all].sort((a, b) => Number(!("workstation" in a && a.workstation)) - Number(!("workstation" in b && b.workstation)));
  $("list").innerHTML = sorted.map(row).join("");
  $("list").classList.toggle("hidden", all.length === 0);
  $("empty").classList.toggle("hidden", all.length > 0);
  const online = friends.filter((f) => f.workstation).length;
  $("summary").textContent = all.length ? `В кампусе: ${online} из ${all.length}` : "Друзья";
}

async function refresh() {
  if (loading) return;
  loading = true;
  $<HTMLButtonElement>("refresh").disabled = true;
  render(settings.friends.filter((l) => !friends.some((f) => f.login === l)));
  try {
    friends = await invoke<FriendInfo[]>("friends_list");
    render();
  } finally {
    loading = false;
    $<HTMLButtonElement>("refresh").disabled = false;
  }
}

$("add-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const input = $<HTMLInputElement>("login");
  const error = $("add-error");
  const button = $<HTMLButtonElement>("add");
  if (!input.value.trim()) return;
  error.classList.add("hidden");
  button.disabled = true;
  try {
    const login = await invoke<string>("friends_add", { login: input.value });
    settings.friends = [...settings.friends, login];
    input.value = "";
    await refresh();
  } catch (err) {
    const code = String(err);
    error.textContent =
      code === "already" ? "Уже в списке"
      : code === "limit" ? "Не больше 30 друзей"
      : code === "http_404" ? "Пир с таким логином не найден"
      : errorText(err);
    error.classList.remove("hidden");
  } finally {
    button.disabled = false;
  }
});

$("list").addEventListener("click", async (e) => {
  const target = e.target as HTMLElement;
  const remove = target.closest<HTMLElement>("[data-remove]");
  if (remove) {
    e.stopPropagation();
    const login = remove.dataset.remove!;
    await invoke("friends_remove", { login });
    settings.friends = settings.friends.filter((l) => l !== login);
    friends = friends.filter((f) => f.login !== login);
    render();
    return;
  }
  const item = target.closest<HTMLElement>("[data-login]");
  if (item) openPlatform(`/profile/${encodeURIComponent(item.dataset.login!)}`);
});

$("refresh").addEventListener("click", refresh);
$("open-settings").addEventListener("click", () => openWindow("settings"));

async function init() {
  renderIcons();
  const info = await getInfo();
  settings = info.settings;
  $("needs-login").classList.toggle("hidden", !!info.login);
  $("content").classList.toggle("hidden", !info.login);
  if (!info.login) return;

  const notify = $<HTMLInputElement>("notify");
  notify.checked = settings.notifyFriends;
  notify.addEventListener("change", async () => {
    settings = { ...settings, notifyFriends: notify.checked };
    await invoke("save_settings", { settings });
  });
  $<HTMLInputElement>("login").focus();
  await refresh();
}

init();
