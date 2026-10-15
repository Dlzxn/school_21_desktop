import { invoke } from "@tauri-apps/api/core";
import {
  $, errorText, escapeHtml, getInfo, hours, num, openPlatform, openWindow, place,
  type Coalition, type Points, type Workstation,
} from "./common";
import { renderIcons } from "./icons";

interface PeerInfo {
  participant: {
    login: string;
    className: string | null;
    parallelName: string | null;
    level: number;
    expValue: number;
    expToNextLevel: number;
    campus: string | null;
    status: string | null;
  };
  workstation: Workstation | null;
  points: Points | null;
  logtime: number | null;
  coalition: Coalition | null;
}

let ownLogin: string | null = null;
let friends: string[] = [];

function show(id: string, visible: boolean) {
  $(id).classList.toggle("hidden", !visible);
}

async function search(login: string) {
  const query = login.trim().split("@")[0].toLowerCase();
  if (!query) return;
  $<HTMLInputElement>("query").value = query;
  show("result", false);
  show("error", false);
  show("spinner", true);
  try {
    render(await invoke<PeerInfo>("peer_lookup", { login: query }));
  } catch (e) {
    $("error").textContent = String(e) === "http_404" ? "Пир с таким логином не найден" : errorText(e);
    show("error", true);
  } finally {
    show("spinner", false);
  }
}

function render(info: PeerInfo) {
  const p = info.participant;
  $("avatar").textContent = p.login.charAt(0).toUpperCase();
  $("peer-login").textContent = p.login;
  $("peer-subtitle").textContent = [p.parallelName, p.className, p.campus].filter(Boolean).join(" · ");

  const placeEl = $("peer-place");
  placeEl.textContent = info.workstation ? place(info.workstation) : "Сейчас не в кампусе";
  placeEl.classList.toggle("on", !!info.workstation);

  const coalition = $("peer-coalition");
  coalition.classList.toggle("hidden", !info.coalition);
  if (info.coalition) {
    const rank = info.coalition.rank != null ? ` <b>#${info.coalition.rank} в трайбе</b>` : "";
    coalition.innerHTML = `🛡 ${escapeHtml(info.coalition.name)}${rank}`;
  }

  $("peer-level").textContent = String(p.level);
  $("peer-xp").textContent = `${num(p.expValue)} XP`;
  show("peer-next-box", p.expToNextLevel > 0);
  $("peer-next-label").textContent = `до ${p.level + 1} уровня`;
  $("peer-next").textContent = `${num(p.expToNextLevel)} XP`;

  const pts = info.points;
  $("peer-points").innerHTML = pts
    ? `<div class="tile accent"><div class="value">${num(pts.coins)}</div><div class="name">коины</div></div>
       <div class="tile"><div class="value">${pts.peerReviewPoints}</div><div class="name">PRP</div></div>
       <div class="tile"><div class="value">${pts.codeReviewPoints}</div><div class="name">CRP</div></div>`
    : "";
  $("peer-logtime").textContent = info.logtime != null ? `Логтайм: ${hours(info.logtime)} ч (в среднем за неделю)` : "";
  $("open-profile").onclick = () => openPlatform(`/profile/${encodeURIComponent(p.login)}`);
  const addFriend = $<HTMLButtonElement>("add-friend");
  const isMe = p.login.toLowerCase() === ownLogin;
  const isFriend = friends.includes(p.login.toLowerCase());
  addFriend.classList.toggle("hidden", isMe);
  addFriend.disabled = isFriend;
  addFriend.textContent = isFriend ? "В друзьях" : "Добавить в друзья";
  addFriend.onclick = async () => {
    addFriend.disabled = true;
    try {
      friends.push(await invoke<string>("friends_add", { login: p.login }));
      addFriend.textContent = "В друзьях";
    } catch (e) {
      addFriend.textContent = String(e) === "limit" ? "Не больше 30 друзей" : String(e) === "already" ? "В друзьях" : errorText(e);
    }
  };
  show("result", true);
}

$("search-form").addEventListener("submit", (e) => {
  e.preventDefault();
  search($<HTMLInputElement>("query").value);
});
$("me").addEventListener("click", () => ownLogin && search(ownLogin));
$("open-settings").addEventListener("click", () => openWindow("settings"));

async function init() {
  renderIcons();
  ownLogin = await invoke<string | null>("own_login");
  friends = (await getInfo()).settings.friends;
  show("needs-login", !ownLogin);
  show("search-form", !!ownLogin);
  $<HTMLInputElement>("query").focus();
}

init();
