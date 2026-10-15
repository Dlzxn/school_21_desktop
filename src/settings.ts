import { invoke } from "@tauri-apps/api/core";
import { $, errorText, getInfo, openExternal, type Settings } from "./common";
import { renderIcons } from "./icons";

const REPO = "https://github.com/Dlzxn/school_21_desktop";

/** Только булевы настройки — их переключают тумблеры data-setting. */
type BoolSetting = { [K in keyof Settings]: Settings[K] extends boolean ? K : never }[keyof Settings];

let settings: Settings;
let savedLogin: string | null = null;

function renderAccount(editing: boolean) {
  $("login-status").textContent = savedLogin ? `Включён для ${savedLogin}` : "Выключен";
  $("login-form").classList.toggle("hidden", !editing);
  $("login-change").classList.toggle("hidden", editing || !savedLogin);
  $("logout-row").classList.toggle("hidden", editing || !savedLogin);
  $("login-cancel").classList.toggle("hidden", !savedLogin);
  $("login-error").classList.add("hidden");
  if (editing) $<HTMLInputElement>("login").value = savedLogin ?? "";
  $<HTMLInputElement>("password").value = "";
}

async function init() {
  const info = await getInfo();
  settings = info.settings;
  savedLogin = info.login;
  $("version").textContent = `Версия ${info.version} · неофициальный клиент платформы`;
  renderAccount(!savedLogin);

  document.querySelectorAll<HTMLInputElement>("input[data-setting]").forEach((input) => {
    const key = input.dataset.setting as BoolSetting;
    input.checked = settings[key];
    input.addEventListener("change", async () => {
      settings = { ...settings, [key]: input.checked };
      await invoke("save_settings", { settings });
    });
  });

  const autostart = $<HTMLInputElement>("autostart");
  autostart.checked = info.autostart;
  autostart.addEventListener("change", async () => {
    try {
      await invoke("set_autostart", { enabled: autostart.checked });
    } catch {
      autostart.checked = !autostart.checked;
    }
  });
}

$("login-change").addEventListener("click", () => renderAccount(true));
$("login-cancel").addEventListener("click", () => renderAccount(false));

$("login-save").addEventListener("click", async () => {
  const login = $<HTMLInputElement>("login").value.trim();
  const password = $<HTMLInputElement>("password").value;
  const button = $<HTMLButtonElement>("login-save");
  const error = $("login-error");
  if (!login || !password) return;
  button.disabled = true;
  button.textContent = "Проверяю…";
  error.classList.add("hidden");
  try {
    await invoke("save_credentials", { login, password });
    savedLogin = login;
    renderAccount(false);
  } catch (e) {
    error.textContent = errorText(e);
    error.classList.remove("hidden");
  } finally {
    button.disabled = false;
    button.textContent = "Проверить и сохранить";
  }
});

$("logout").addEventListener("click", async () => {
  await invoke("clear_credentials");
  savedLogin = null;
  renderAccount(true);
});

let pendingUpdate: { version: string; url: string; installable: boolean } | null = null;

$("check-update").addEventListener("click", async () => {
  const status = $("update-status");
  // Второе нажатие после «Доступна версия» — поставить.
  if (pendingUpdate) {
    if (!pendingUpdate.installable) {
      await openExternal(pendingUpdate.url);
      return;
    }
    status.textContent = `Скачиваю версию ${pendingUpdate.version}… приложение перезапустится`;
    try {
      await invoke("install_update");
    } catch {
      status.textContent = "Не удалось обновить — откройте страницу релизов";
      pendingUpdate = { ...pendingUpdate, installable: false, url: `${REPO}/releases/latest` };
    }
    return;
  }
  status.textContent = "Проверяю…";
  try {
    pendingUpdate = await invoke<{ version: string; url: string; installable: boolean } | null>("check_update");
    if (pendingUpdate) {
      status.textContent = pendingUpdate.installable
        ? `Доступна версия ${pendingUpdate.version} — нажмите, чтобы обновить`
        : `Доступна версия ${pendingUpdate.version} — нажмите, чтобы открыть страницу загрузки`;
    } else {
      status.textContent = "У вас последняя версия";
    }
  } catch {
    status.textContent = "Не удалось проверить обновления";
  }
});

$("open-site").addEventListener("click", () => openExternal("https://sc21-apps.github.io"));
$("open-repo").addEventListener("click", () => openExternal(REPO));
$("report-bug").addEventListener("click", () => openExternal(`${REPO}/issues`));
$("author").addEventListener("click", () => openExternal("https://github.com/Dlzxn"));

renderIcons();
init();
