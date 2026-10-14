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

$("check-update").addEventListener("click", async () => {
  const status = $("update-status");
  status.textContent = "Проверяю…";
  try {
    const update = await invoke<{ version: string; url: string } | null>("check_update");
    if (update) {
      status.textContent = `Доступна версия ${update.version} — открываю страницу загрузки`;
      await openExternal(update.url);
    } else {
      status.textContent = "У вас последняя версия";
    }
  } catch {
    status.textContent = "Не удалось проверить обновления";
  }
});

$("open-repo").addEventListener("click", () => openExternal(REPO));
$("report-bug").addEventListener("click", () => openExternal(`${REPO}/issues`));
$("author").addEventListener("click", () => openExternal("https://github.com/Dlzxn"));

renderIcons();
init();
