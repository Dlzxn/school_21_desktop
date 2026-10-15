import { invoke } from "@tauri-apps/api/core";

export interface Settings {
  notifyPlatform: boolean;
  notifyAgenda: boolean;
  notifyEvents: boolean;
  notifyFriends: boolean;
  autoUpdate: boolean;
  closeToTray: boolean;
  friends: string[];
}

export interface Workstation { clusterName: string; row: string; number: number }
export interface Points { peerReviewPoints: number; codeReviewPoints: number; coins: number }
export interface Coalition { name: string; rank: number | null }
export interface AppInfo {
  version: string;
  login: string | null;
  settings: Settings;
  autostart: boolean;
}

export const getInfo = () => invoke<AppInfo>("get_info");
export const openExternal = (url: string) => invoke("open_external", { url });
export const openWindow = (label: string) => invoke("open_window", { label });
export const openPlatform = (path: string) => invoke("open_platform", { path });

export const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

/** 12345 → «12 345». */
export const num = (n: number) => n.toLocaleString("ru-RU");

export const place = (w: Workstation) => `${w.clusterName} · ${w.row.toUpperCase()}${w.number}`;

export const hours = (h: number) => h.toLocaleString("ru-RU", { maximumFractionDigits: 1 });

/** Человеческое сообщение по коду ошибки из Rust. */
export function errorText(e: unknown): string {
  const code = String(e);
  if (code === "http_404") return "Не найдено";
  if (code === "http_429") return "Слишком много запросов, попробуйте через минуту";
  if (code === "invalid_credentials") return "Неверный логин или пароль";
  if (code === "not_configured") return "Сначала включите автовход в настройках";
  if (code.startsWith("network")) return "Нет связи с сервером";
  return "Не удалось получить данные";
}

export function escapeHtml(text: string): string {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}
