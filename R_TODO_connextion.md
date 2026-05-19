# Peer Review — Momentum App

> Audit de sécurité, logique de connexion, et organisation architecturale  
> Date : 2026-05-20 | Reviewer : Claude Sonnet 4.6

---

## Table des matières

1. [Vue d'ensemble du projet](#1-vue-densemble)
2. [Tests simulés — Auth & Connexion](#2-tests-simulés--auth--connexion)
3. [Tests simulés — Refresh Token](#3-tests-simulés--refresh-token)
4. [Tests simulés — WebSocket](#4-tests-simulés--websocket)
5. [Tests simulés — Stream WHIP](#5-tests-simulés--stream-whip)
6. [Tests simulés — Déconnexion & Logout](#6-tests-simulés--déconnexion--logout)
7. [Bugs & Failles identifiées](#7-bugs--failles-identifiées)
8. [Analyse architecturale](#8-analyse-architecturale)
9. [Plan de réorganisation](#9-plan-de-réorganisation)
10. [Liste des fonctions à déplacer](#10-liste-des-fonctions-à-déplacer)

---

## 1. Vue d'ensemble

**Stack :** Tauri 2 + React 18 + TypeScript + Rust  
**Flow principal :**

```
Login (OAuth PKCE) → WS Connect → LobbySetup → StreamSetup (WHIP) → WaitingForStart → Racing → Finished
```

**Fichiers Rust critiques :**

- `auth/oauth.rs` — PKCE, échange de code, callback deep-link
- `auth/refresh.rs` — refresh loop
- `auth/token_store.rs` — persistence via tauri-plugin-store
- `ws/client.rs` — connexion WS + reconnect loop
- `ws/handler.rs` — parsing des messages serveur
- `commands.rs` — Tauri commands exposées au webview
- `state.rs` — état global partagé (Mutex)
- `lib.rs` — setup, restore_session

---

## 2. Tests simulés — Auth & Connexion

### ✅ TEST-AUTH-01 : Login nominal (premier démarrage)

**Scénario :** L'utilisateur clique "Se connecter", le browser s'ouvre, il s'authentifie, le callback deep-link arrive.

**Trace :**

1. `handleLogin()` → `openLogin()` → `open_browser_login()` ✅
2. PKCE généré (`generate_pkce()`), verifier stocké dans `PENDING_PKCE_VERIFIER` (Mutex static) ✅
3. Browser ouvre `https://weyrd.space/api/auth/desktop?client_id=tauri_desktop&...` ✅
4. Deep-link `momentum://auth/callback?code=XXX` reçu ✅
5. `handle_callback()` consomme le verifier, échange le code ✅
6. Tokens sauvegardés via `TokenStore` ✅
7. `emit_auth_state(Authenticated)` → webview reçoit `auth:state` ✅
8. `token_refresh_loop` + `ws_connect_loop` lancés en background ✅

**Résultat : PASS**

---

### ❌ TEST-AUTH-02 : Double-clic sur "Se connecter" (race condition PKCE)

**Scénario :** L'utilisateur clique deux fois rapidement.

**Trace :**

1. Premier clic → PKCE verifier A stocké
2. Deuxième clic → `open_browser_login()` appelé une 2ème fois → **PKCE verifier A écrasé par B**
3. Deux fenêtres browser s'ouvrent (pas de guard)
4. Premier callback arrive avec code A → verifier B utilisé pour l'échange → **401 invalid_grant**
5. `emit_auth_state(Unauthenticated)` émis
6. Deuxième callback arrive → verifier B déjà consommé (pris par le premier) ou écrasé → état incohérent

**Résultat : FAIL 🔴**  
**Faille :** Aucune protection contre un double `open_login`. Le verifier static peut être écrasé entre deux appels concurrents.

---

### ❌ TEST-AUTH-03 : Callback sans verifier en attente (replay attack)

**Scénario :** Deep-link `momentum://auth/callback?code=XXX` reçu sans login en cours.

**Trace :**

1. `PENDING_PKCE_VERIFIER` est `None`
2. Code détecté : `eprintln!("no pending PKCE verifier — possible replay attack, ignoring")` → return ✅

**Résultat : PASS** (géré correctement)

---

### ❌ TEST-AUTH-04 : Callback avec paramètre `error`

**Scénario :** L'utilisateur refuse les permissions, le serveur renvoie `momentum://auth/callback?error=access_denied`.

**Trace :**

1. `params.get("error")` détecté → `emit_auth_state(Unauthenticated)` ✅
2. **Mais** : le PKCE verifier n'est pas consommé/nettoyé → reste en mémoire
3. Un prochain login va écraser le verifier stale → OK en pratique, mais le verifier "flotte"

**Résultat : WARN 🟡**  
**Faille mineure :** Après un refus OAuth, `PENDING_PKCE_VERIFIER` n'est pas remis à `None`. Pas critique mais laisse un artefact.

---

### ✅ TEST-AUTH-05 : Restore session au démarrage (token valide)

**Scénario :** L'app redémarre avec des tokens valides en store.

**Trace :**

1. `restore_session()` appelé dans `setup()`
2. `store.load()` → tokens trouvés ✅
3. `store.is_expired()` → false → `stored.user` utilisé directement ✅
4. `fetch_current_lobby()` → appel HTTP avec Bearer token ✅
5. `guard.app_state = StreamSetup` si lobby trouvé, sinon `Connecting` ✅
6. `emit_auth_state(Authenticated)` ✅
7. `token_refresh_loop` + `ws_connect_loop` lancés ✅

**Résultat : PASS**

---

### ✅ TEST-AUTH-06 : Restore session au démarrage (token expiré)

**Scénario :** L'app redémarre, l'access token est expiré mais le refresh token est valide.

**Trace :**

1. `store.is_expired()` → true
2. `do_refresh(&stored.tokens.refresh_token)` appelé ✅
3. Nouveaux tokens persistés via `store.update_tokens()` ✅
4. Session restaurée ✅

**Résultat : PASS**

---

### ❌ TEST-AUTH-07 : Restore session (refresh token expiré aussi)

**Scénario :** Les deux tokens sont expirés.

**Trace :**

1. `do_refresh()` → 401 Unauthorized
2. `store.clear().ok()` ✅
3. `emit_auth_state(Unauthenticated)` ✅
4. `return` — la session n'est pas restaurée ✅

**Mais** : le webview a déjà reçu un `getLobbyState()` via `useAppState` qui retourne un état potentiellement `Connecting` depuis le Rust, **avant** que `restore_session` ait fini. Il y a une race condition entre `getLobbyState()` (appelé dans `useEffect` du hook) et la fin de `restore_session`.

**Résultat : FAIL 🔴**  
**Faille :** `getLobbyState()` appelé dès le montage du composant, mais `restore_session` est async et peut prendre du temps. Le front peut afficher l'écran "Idle" ou "Connecting" pendant quelques centaines de ms avant que le vrai état soit émis.

---

## 3. Tests simulés — Refresh Token

### ✅ TEST-REFRESH-01 : Refresh nominal

**Scénario :** Le token va expirer dans 70s, la marge est de 60s.

**Trace :**

1. `expires_in` = 70s → `sleep = 70 - 60 = 10s`
2. Après 10s, `refresh_access_token()` appelé ✅
3. Nouveau token persisté ✅
4. Loop reprend ✅

**Résultat : PASS**

---

### ❌ TEST-REFRESH-02 : Deux refresh loops simultanées

**Scénario :** Login initial + restore_session au démarrage (app déjà authentifiée et l'utilisateur re-login en même temps).

**Trace :**

1. `restore_session` lance `token_refresh_loop` ✅
2. Si l'utilisateur clique "Login" pendant restore → `handle_callback` lance **un 2ème** `token_refresh_loop`
3. **Deux loops tournent en parallèle**, chacune rafraîchissant le token indépendamment
4. Double-refresh sur le même refresh_token → le 2ème appel peut obtenir un 401 (rotation de refresh token)

**Résultat : FAIL 🔴**  
**Faille :** Aucun guard contre le lancement de plusieurs `token_refresh_loop`. Chaque callback auth démarre une nouvelle loop sans vérifier si une tourne déjà.

---

### ✅ TEST-REFRESH-03 : Refresh token révoqué pendant la loop

**Scénario :** Admin révoque le token côté serveur.

**Trace :**

1. `refresh_access_token()` → 401
2. `logout_and_notify()` → `store.clear()` + `emit_auth_state(Unauthenticated)` ✅
3. Loop return ✅

**Résultat : PASS**

---

### ❌ TEST-REFRESH-04 : `is_expired()` comparaison à ZERO

**Scénario :** Token qui expire dans exactement 0s (ou parse error sur `expires_at`).

**Trace :**

1. `seconds_until_expiry()` → parse error ou 0s → retourne `Duration::ZERO`
2. `is_expired()` → `self.time_until_expiry() == Duration::ZERO` ✅
3. Dans le refresh loop : `sleep = 0s - 60s` → `saturating_sub` → `Duration::ZERO` ✅ (pas de underflow)
4. Refresh immédiat ✅

**Mais** : si `expires_at` est malformé, `is_expired()` retourne `true` même si le token est valide, forçant un refresh inutile au démarrage.

**Résultat : WARN 🟡**

---

## 4. Tests simulés — WebSocket

### ✅ TEST-WS-01 : Connexion WS nominale

**Scénario :** Token valide, serveur accessible.

**Trace :**

1. `ws_url(token)` construit l'URL WSS ✅
2. `emit_ws_status(Connecting)` ✅
3. `connect_async()` → succès
4. `emit_ws_status(Connected)` ✅
5. `guard.app_state` passe de `Connecting` à `Idle` si c'était `Connecting` ✅

**Résultat : PASS**

---

### ✅ TEST-WS-02 : Reconnexion avec backoff exponentiel

**Scénario :** Connexion perdue.

**Trace :**

1. Boucle interne break → `emit_ws_status(Disconnected)`
2. `tokio::time::sleep(backoff)` (1s, 2s, 4s... max 30s) ✅
3. Reconnexion tentée ✅
4. Backoff reset à 1s si connexion OK ✅

**Résultat : PASS**

---

### ❌ TEST-WS-03 : WS connect loop lancée plusieurs fois

**Scénario :** Même problème que refresh — deux logins simultanés.

**Trace :**

1. Première `ws_connect_loop` lancée → `guard.ws_cmd_tx = Some(tx1)`
2. Deuxième `ws_connect_loop` lancée → `guard.ws_cmd_tx = Some(tx2)` **écrase tx1**
3. La première loop continue de tourner mais le sender vers elle est perdu
4. Les commandes WS vont vers la 2ème loop uniquement
5. **Deux connexions WS actives** vers le serveur avec le même token

**Résultat : FAIL 🔴**  
**Faille :** Même cause racine que TEST-REFRESH-02 : pas de guard contre les loops multiples.

---

### ❌ TEST-WS-04 : `notify_stream_stopped` ne notifie pas le WS

**Scénario :** L'utilisateur stoppe le stream manuellement.

**Trace :**

1. `handleStopStream()` côté front → `notifyStreamStopped()` → invoke Rust `notify_stream_stopped`
2. `notify_stream_stopped` Rust : met à jour l'état, émet `app:state` ✅
3. **Mais** : n'envoie PAS de message `stream_stopped` via WebSocket !
4. Le serveur n'est jamais informé que le stream s'est arrêté proprement
5. `WsCommand::StreamStopped { lobby_id }` existe dans `ws/commands.rs` mais **n'est jamais utilisé**

**Résultat : FAIL 🔴**  
**Faille critique :** Le serveur n'est pas notifié du stop de stream. Le `WsCommand::StreamStopped` est défini mais jamais envoyé.

---

### ✅ TEST-WS-05 : Message `Ping` reçu

**Scénario :** Serveur envoie un ping.

**Trace :**

1. `ServerMessage::Ping` matché → `{}` (no-op) ✅
2. Pas de pong envoyé — WebSocket niveau applicatif, le niveau transport gère les pings ✅

**Résultat : PASS** (acceptable)

---

### ❌ TEST-WS-06 : Message inconnu reçu

**Scénario :** Serveur envoie un type de message non géré.

**Trace :**

1. `serde_json::from_str()` → `Err` (tag inconnu)
2. `eprintln!("[ws] parse error")` → connexion continue ✅

**Résultat : PASS** (dégradation gracieuse)

---

## 5. Tests simulés — Stream WHIP

### ✅ TEST-WHIP-01 : Publication stream nominale

**Scénario :** L'utilisateur sélectionne un écran, clique Publier.

**Trace :**

1. `startPreview()` → `getDisplayMedia()` → stream acquis ✅
2. `handlePublish()` → `WhipClient.start(whip_url, stream)` ✅
3. SDP offer créé, ICE gathering attendu ✅
4. POST vers MediaMTX → réponse 201 avec Location header ✅
5. `notifyStreamReady(lobby_id)` → invoke Rust ✅
6. `onStreamReady(client)` → `whipRef.current = client` ✅

**Résultat : PASS**

---

### ❌ TEST-WHIP-02 : Timeout ICE gathering

**Scénario :** Pas d'interface réseau, ICE gathering ne complète jamais.

**Trace :**

1. `setTimeout` après 10s → `reject(new Error("[whip] ICE gathering timed out after 10s"))` ✅
2. `client.stop()` appelé dans le catch ✅
3. `setError("Erreur de connexion au stream.")` ✅

**Résultat : PASS**

---

### ❌ TEST-WHIP-03 : L'utilisateur stoppe le partage d'écran avant de publier

**Scénario :** L'utilisateur clique "Partager l'écran" puis ferme le dialog de partage.

**Trace :**

1. `"ended"` event sur la videoTrack → `setIsPreviewing(false)` + `streamRef.current = null` ✅
2. Bouton Publier désactivé ✅

**Résultat : PASS**

---

### ❌ TEST-WHIP-04 : `WhipClient.stop()` appelé pendant ICE gathering

**Scénario :** Utilisateur annule pendant la publication (très rapide).

**Trace :**

1. `stop()` appelé → `this.pc.close()` → `this.pc = null`
2. `_gatherCompleteOffer` a un `onicegatheringstatechange` callback qui tente `this.pc?.iceGatheringState` → `this.pc` est null → `?.` → `undefined` → condition fausse
3. Promise ne resolve jamais, timeout fire après 10s → `reject` → mais `whipRef` déjà null
4. L'erreur arrive dans le catch de `handlePublish` qui tente `client.stop()` une 2ème fois → no-op car `this.pc = null` ✅

**Résultat : PASS** (comportement tolérant)

---

### ❌ TEST-WHIP-05 : `stop()` DELETE du resource URL échoue silencieusement

**Scénario :** Le serveur MediaMTX est injoignable au moment du stop.

**Trace :**

1. `fetch(resourceUrl, { method: "DELETE" }).catch(() => {})` → erreur ignorée ✅
2. La connexion WebRTC est fermée localement ✅
3. MediaMTX détecte la fermeture via ICE ✅

**Résultat : PASS** (best-effort acceptable)

---

## 6. Tests simulés — Déconnexion & Logout

### ✅ TEST-LOGOUT-01 : Logout nominal

**Scénario :** L'utilisateur clique "Se déconnecter" (via le bouton ⚙ qui appelle `onSettingsClick → handleLogout`).

**Trace :**

1. `handleLogout()` → `whipRef.current?.stop()` (stream stoppé) ✅
2. `patch(initialState)` → UI reset ✅
3. `logout()` → invoke Rust `logout`
4. Rust : `TokenStore.clear()` ✅
5. `guard.ws_cmd_tx = None` → la prochaine iteration du WS loop → `rx.recv()` retourne `None` → `WsCommand::Disconnect` matché → `Message::Close` envoyé ✅
6. `emit_auth_state(Unauthenticated)` ✅
7. DELETE `/api/v1/auth/desktop/logout` avec refresh_token ✅

**Résultat : PASS**

---

### ❌ TEST-LOGOUT-02 : Logout alors que WS est en train de se reconnecter (backoff sleep)

**Scénario :** La connexion WS est tombée, on est dans le sleep de backoff, et l'utilisateur se déconnecte.

**Trace :**

1. `logout()` Rust → `guard.ws_cmd_tx = None` ✅
2. Mais la WS loop est dans `tokio::time::sleep(backoff)` — elle n'est pas en train d'écouter le canal
3. Après le sleep, `TokenStore.get_access_token()` → `None` (tokens cleared)
4. La loop return avec `eprintln!("[ws] no access token, aborting connect loop")` ✅

**Résultat : PASS** (se résout correctement, juste avec un délai)

---

### ❌ TEST-LOGOUT-03 : Race condition entre `onAuthState` et `getLobbyState`

**Scénario :** Au démarrage, `getLobbyState()` et `onAuthState` (depuis restore_session) arrivent dans des ordres différents.

**Trace :**

1. `useEffect` (premier) : `getLobbyState()` → retourne `{ app_state: Unauthenticated }` car restore_session n'a pas encore fini
2. `useEffect` (second) : listeners setup → `onAuthState` handler enregistré
3. `restore_session` finit → `emit_auth_state(Authenticated)` → handler appelé → `patch({ appState: Connecting, user })` ✅

**Résultat : PASS** (le design event-driven rattrape la race condition, mais le flash Unauthenticated → Connecting est visible)

---

## 7. Bugs & Failles identifiées

### 🔴 BUG-01 — `WsCommand::StreamStopped` jamais envoyé

**Fichier :** `commands.rs` → `notify_stream_stopped`  
**Problème :** La fonction met à jour l'état local et émet un event Tauri mais n'envoie **jamais** de message WS `stream_stopped` au serveur. Le type `WsCommand::StreamStopped { lobby_id }` est défini dans `ws/commands.rs` mais unused.  
**Impact :** Le serveur backend ne sait jamais que le stream s'est arrêté voluntairement → lobby potentiellement dans un état inconsistant côté backend.  
**Fix :**

```rust
// Dans notify_stream_stopped :
let lobby_id = {
    let guard = state.lock().map_err(|e| e.to_string())?;
    guard.lobby.as_ref().map(|l| l.lobby_id.clone())
};
if let Some((tx, id)) = sender.zip(lobby_id) {
    let _ = tx.try_send(WsCommand::StreamStopped { lobby_id: id });
}
```

---

### 🔴 BUG-02 — Multiples `token_refresh_loop` et `ws_connect_loop` simultanées

**Fichiers :** `auth/oauth.rs` → `handle_callback`, `lib.rs` → `restore_session`  
**Problème :** Chaque succès d'auth lance une nouvelle `token_refresh_loop` et `ws_connect_loop` sans vérifier si une tourne déjà.  
**Impact :** Doubles refresh → rotation de tokens cassée. Double WS → deux connexions actives côté serveur.  
**Fix :** Utiliser un `AtomicBool` ou un `tokio::sync::Mutex<Option<JoinHandle>>` pour tracker si les loops sont actives.

---

### 🔴 BUG-03 — Race condition sur `getLobbyState` vs `restore_session`

**Fichier :** `hooks/useAppState.ts`  
**Problème :** `getLobbyState()` est appelé immédiatement au mount, avant que `restore_session` ait terminé. L'état initial peut être stale.  
**Impact :** Flash UI de l'état "Unauthenticated" même si l'utilisateur était connecté.  
**Fix :** Soit retarder `getLobbyState` jusqu'à recevoir `auth:state`, soit ajouter un état `"loading"` initial.

---

### 🟡 BUG-04 — Double-clic login (PKCE verifier écrasé)

**Fichier :** `auth/oauth.rs` → `open_browser_login`  
**Problème :** Aucune protection contre un double appel. Le verifier est écrasé.  
**Fix :**

```rust
pub fn open_browser_login(app: &AppHandle) -> Result<(), String> {
    let mut pending = PENDING_PKCE_VERIFIER.lock().map_err(|e| e.to_string())?;
    if pending.is_some() {
        return Err("login already in progress".into());
    }
    // ... reste du code
}
```

Côté front, le bouton est déjà `disabled` pendant `Connecting` → protection partielle ✅ mais pas côté Rust.

---

### 🟡 BUG-05 — `PENDING_PKCE_VERIFIER` non nettoyé après callback erreur

**Fichier :** `auth/oauth.rs` → `handle_callback`  
**Problème :** Si le serveur renvoie `?error=...`, le verifier n'est pas consommé.  
**Fix :** Ajouter `pending.take()` dans le bloc erreur.

---

### 🟡 BUG-06 — `formatElapsed` dans `Racing.tsx` (dead code)

**Fichier :** `src/components/Racing.tsx`  
**Problème :** La fonction `formatElapsed` appelle `useSyncExternalStore` hors d'un composant React (violation des règles des hooks). Elle est commentée avec `void formatElapsed` mais reste dans le fichier.  
**Fix :** Supprimer complètement la fonction.

---

### 🟡 BUG-07 — `get_app_state` command non enregistrée dans `lib.rs`

**Fichier :** `lib.rs` → `invoke_handler`  
**Problème :** `commands::get_app_state` est défini dans `commands.rs` et exposé dans le front (`tauri.ts`), mais **absent du `generate_handler![]`** dans `lib.rs`. Aussi, `commands::get_lobby_state` est absent.  
**Impact :** `invoke("get_app_state")` et `invoke("get_lobby_state")` échouent en runtime.

> ⚠️ **Note :** `get_lobby_state` est utilisé dans `useAppState.ts` et doit être dans le handler.

**Fix :**

```rust
.invoke_handler(tauri::generate_handler![
    commands::get_app_state,       // manquant
    commands::open_login,
    commands::get_current_user,
    commands::logout,
    commands::notify_stream_ready,
    commands::notify_stream_stopped,
    commands::get_lobby_state,     // manquant
])
```

---

### 🟡 BUG-08 — `ApiResponse<T>` défini dans trois fichiers différents

**Fichiers :** `state.rs`, `auth/oauth.rs`, `auth/refresh.rs`  
**Problème :** La struct `ApiResponse<T>` est dupliquée 3 fois localement.  
**Fix :** La déclarer une seule fois dans `state.rs` (déjà public) et l'importer.

---

### 🟡 BUG-09 — `src-tauri/src/build.rs` en double

**Fichier :** `src-tauri/build.rs` (racine) ET `src-tauri/src/build.rs`  
**Problème :** Il y a deux `build.rs`. Cargo utilise uniquement `src-tauri/build.rs` (à la racine du crate). Le `src/build.rs` est du **dead code compilé dans la librairie** — ce qui est incorrect.  
**Fix :** Supprimer `src-tauri/src/build.rs`.

---

### 🟢 NOTE-01 — `csp: null` dans `tauri.conf.json`

**Fichier :** `src-tauri/tauri.conf.json`  
**Observation :** CSP désactivée (`"csp": null`). Acceptable en développement, mais à sécuriser en production avec une CSP stricte qui autorise uniquement les domaines connus (MediaMTX WHIP URL, backend API).

---

### 🟢 NOTE-02 — `//TODO: revoir ce fichier` dans `whip.ts`

**Fichier :** `src/stream/whip.ts`  
**Observation :** TODO laissé. Le fichier est globalement propre — supprimer le commentaire et les anciens commentaires de doc générique.

---

### 🟢 NOTE-03 — `tokio::time::sleep(300ms)` dans `handle_callback`

**Fichier :** `auth/oauth.rs`  
**Observation :** Sleep de 300ms avant d'émettre l'état authentifié. Probablement un workaround pour un timing issue. À investiguer et remplacer par un vrai mécanisme de synchronisation.

---

## 8. Analyse architecturale

### Vue d'ensemble actuelle (Rust)

```
src-tauri/src/
├── main.rs          ✅ minimal, correct
├── lib.rs           ⚠️  setup + restore_session (trop grosse)
├── state.rs         ✅ types partagés, bien placés
├── config.rs        ✅ constantes centralisées
├── events.rs        ✅ noms d'events centralisés
├── commands.rs      ⚠️  logique métier mélangée aux commandes
├── lobby.rs         ✅ bien isolé
├── auth/
│   ├── mod.rs       ✅
│   ├── oauth.rs     🔴 trop gros (PKCE + exchange + browser + callback + helpers)
│   ├── refresh.rs   ✅ raisonnablement propre
│   └── token_store.rs ✅ bien isolé
├── ws/
│   ├── mod.rs       ✅
│   ├── client.rs    ⚠️  `emit_ws_status` définie ici mais utilisée globalement
│   ├── commands.rs  ✅
│   └── handler.rs   ✅
└── stream/
    ├── mod.rs       ✅
    ├── handler.rs   ✅ (trait bien défini pour v2)
    └── whip.rs      🔴 entièrement commenté = dead code
```

### Vue d'ensemble actuelle (Frontend)

```
src/
├── App.tsx          ⚠️  DevToolbar inline (devrait être extrait)
├── main.tsx         ✅
├── types.ts         ✅ bien centralisé
├── hooks/
│   └── useAppState.ts  ⚠️  `matchAuthState` helper devrait être dans un util
├── lib/
│   ├── events.ts    ✅
│   └── tauri.ts     ✅ bonne abstraction
├── stream/
│   └── whip.ts      ✅
└── components/
    ├── Header.tsx       ✅
    ├── TitleBar.tsx     ✅
    ├── Login.tsx        ✅
    ├── Idle.tsx         ✅
    ├── StreamSetup.tsx  ✅
    ├── WaitingForStart.tsx  ⚠️  exporte LivePill + LobbyBadge (couplage)
    ├── Racing.tsx       ⚠️  importe LivePill/LobbyBadge depuis WaitingForStart
    └── StopModal.tsx    ✅
```

---

## 9. Plan de réorganisation

### Rust — Priorité haute

#### R1 : Scinder `auth/oauth.rs`

Le fichier fait trop de choses. Proposé :

```
auth/
├── pkce.rs          # generate_pkce(), PENDING_PKCE_VERIFIER, build_auth_url()
├── exchange.rs      # exchange_code(), TokenExchangeRequest, TokenResponse structs
├── callback.rs      # handle_callback(), parse_query_params()
├── browser.rs       # open_browser_login()
├── payload.rs       # AuthStatePayload, AuthUser, emit_auth_state()
├── refresh.rs       (inchangé)
├── token_store.rs   (inchangé)
└── mod.rs           # re-exports
```

#### R2 : Scinder `lib.rs`

`restore_session` est une fonction métier complexe dans le fichier de setup.

```rust
// Extraire vers :
// src-tauri/src/session.rs
pub async fn restore_session(app: AppHandle, shared_state: SharedState) { ... }
```

#### R3 : Déplacer `emit_ws_status` dans `ws/events.rs` (nouveau fichier)

Actuellement dans `ws/client.rs` mais utilisée conceptuellement comme un utilitaire WS global.

#### R4 : Supprimer `src-tauri/src/build.rs` (dead code)

#### R5 : Centraliser `ApiResponse<T>`

Supprimer les 3 définitions locales, utiliser `crate::state::ApiResponse<T>`.

### Frontend — Priorité basse

#### F1 : Extraire `LivePill` et `LobbyBadge`

Ces composants sont partagés entre `WaitingForStart` et `Racing`. Mauvaise pratique d'importer depuis un composant frère.

#### F2 : Extraire `DevToolbar` de `App.tsx`

Le composant de dev devrait être dans un fichier dédié.

#### F3 : Extraire `matchAuthState` dans `src/lib/utils.ts`

---

## 10. Liste des fonctions à déplacer

### Rust

| Fonction / Struct                                 | Fichier actuel                      | Fichier cible                         | Priorité   |
| ------------------------------------------------- | ----------------------------------- | ------------------------------------- | ---------- |
| `generate_pkce()`                                 | `auth/oauth.rs`                     | `auth/pkce.rs` (nouveau)              | 🟡 MOYENNE |
| `PENDING_PKCE_VERIFIER` static                    | `auth/oauth.rs`                     | `auth/pkce.rs` (nouveau)              | 🟡 MOYENNE |
| `build_auth_url()`                                | `auth/oauth.rs`                     | `auth/pkce.rs` (nouveau)              | 🟡 MOYENNE |
| `open_browser_login()`                            | `auth/oauth.rs`                     | `auth/browser.rs` (nouveau)           | 🟡 MOYENNE |
| `handle_callback()`                               | `auth/oauth.rs`                     | `auth/callback.rs` (nouveau)          | 🟡 MOYENNE |
| `exchange_code()` + structs                       | `auth/oauth.rs`                     | `auth/exchange.rs` (nouveau)          | 🟡 MOYENNE |
| `parse_query_params()`                            | `auth/oauth.rs`                     | `auth/callback.rs` ou `auth/utils.rs` | 🟢 BASSE   |
| `AuthStatePayload`, `AuthUser`, `emit_auth_state` | `auth/oauth.rs`                     | `auth/payload.rs` (nouveau)           | 🟡 MOYENNE |
| `ApiResponse<T>` (duplicate)                      | `auth/oauth.rs` + `auth/refresh.rs` | `state.rs` (déjà là)                  | 🔴 HAUTE   |
| `restore_session()`                               | `lib.rs`                            | `session.rs` (nouveau)                | 🟡 MOYENNE |
| `emit_ws_status()`                                | `ws/client.rs`                      | `ws/events.rs` (nouveau)              | 🟢 BASSE   |
| `src-tauri/src/build.rs`                          | `src-tauri/src/build.rs`            | SUPPRIMER                             | 🔴 HAUTE   |

### Frontend

| Fonction / Composant | Fichier actuel                   | Fichier cible                                | Priorité   |
| -------------------- | -------------------------------- | -------------------------------------------- | ---------- |
| `LivePill`           | `components/WaitingForStart.tsx` | `components/shared/LivePill.tsx` (nouveau)   | 🟡 MOYENNE |
| `LobbyBadge`         | `components/WaitingForStart.tsx` | `components/shared/LobbyBadge.tsx` (nouveau) | 🟡 MOYENNE |
| `DevToolbar`         | `App.tsx`                        | `components/dev/DevToolbar.tsx` (nouveau)    | 🟢 BASSE   |
| `matchAuthState()`   | `hooks/useAppState.ts`           | `lib/utils.ts` (nouveau)                     | 🟢 BASSE   |
| `formatElapsed()`    | `components/Racing.tsx`          | **SUPPRIMER** (dead code)                    | 🔴 HAUTE   |

---

## Résumé des résultats

| Catégorie        | PASS   | FAIL 🔴 | WARN 🟡 |
| ---------------- | ------ | ------- | ------- |
| Auth & Connexion | 3      | 2       | 2       |
| Refresh Token    | 2      | 1       | 1       |
| WebSocket        | 3      | 2       | 0       |
| Stream WHIP      | 3      | 0       | 0       |
| Logout           | 2      | 0       | 1       |
| **Total**        | **13** | **5**   | **4**   |

### Bugs critiques à patcher en priorité

1. **BUG-07** — `get_app_state` et `get_lobby_state` absents du `invoke_handler` → crash runtime garanti
2. **BUG-01** — `WsCommand::StreamStopped` jamais envoyé → serveur jamais notifié
3. **BUG-02** — Multiples refresh/WS loops simultanées → comportement indéfini
4. **BUG-09** — `src/build.rs` doublon → dead code dans la librairie
5. **BUG-08** — `ApiResponse<T>` tripliqué → DRY violation

---

_Review terminée. Aucune modification du code source effectuée — ce document est un audit pur._
